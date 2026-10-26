//! Accounts. Offline ("cracked") accounts are just a name plus a deterministic UUID.
//! Microsoft accounts use the OAuth device-code flow, so no embedded browser or local
//! redirect server is needed and the launcher stays tiny.

use crate::config::Paths;
use crate::util::*;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Account {
    pub id: String,
    pub name: String,
    /// "offline" or "microsoft"
    pub kind: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Store {
    pub accounts: Vec<Account>,
    pub selected: String,
}

pub fn load(paths: &Paths) -> Store {
    read_json_file(&paths.accounts_file())
}

pub fn save(paths: &Paths, store: &Store) -> Result<()> {
    write_json_file(&paths.accounts_file(), store)
}

/// What the UI is allowed to see (never tokens).
pub fn public(store: &Store) -> Value {
    let list: Vec<Value> = store
        .accounts
        .iter()
        .map(|a| json!({ "id": a.id, "name": a.name, "kind": a.kind }))
        .collect();
    json!({ "accounts": list, "selected": store.selected })
}

pub fn offline_uuid(name: &str) -> String {
    let mut b = md5::compute(format!("OfflinePlayer:{name}").as_bytes()).0;
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{:02x}", x)).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

pub fn add_offline(paths: &Paths, name: &str) -> Result<Store> {
    let name = name.trim();
    if name.len() < 3
        || name.len() > 16
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        bail!("Usernames are 3-16 letters, numbers or underscores");
    }
    let mut store = load(paths);
    let id = offline_uuid(name);
    if !store.accounts.iter().any(|a| a.id == id) {
        store.accounts.push(Account {
            id: id.clone(),
            name: name.to_string(),
            kind: "offline".into(),
            ..Default::default()
        });
    }
    store.selected = id;
    save(paths, &store)?;
    Ok(store)
}

pub fn select(paths: &Paths, id: &str) -> Result<Store> {
    let mut store = load(paths);
    if store.accounts.iter().any(|a| a.id == id) {
        store.selected = id.to_string();
        save(paths, &store)?;
    }
    Ok(store)
}

pub fn remove(paths: &Paths, id: &str) -> Result<Store> {
    let mut store = load(paths);
    store.accounts.retain(|a| a.id != id);
    if store.selected == id {
        store.selected = store.accounts.first().map(|a| a.id.clone()).unwrap_or_default();
    }
    save(paths, &store)?;
    Ok(store)
}

pub fn selected(store: &Store) -> Option<Account> {
    store
        .accounts
        .iter()
        .find(|a| a.id == store.selected)
        .or_else(|| store.accounts.first())
        .cloned()
}

// ---- Microsoft ----------------------------------------------------------------------

const TENANT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const SCOPE: &str = "XboxLive.signin offline_access";

fn post_form(url: &str, form: &[(&str, &str)]) -> Result<Value> {
    match http().post(url).send_form(form) {
        Ok(r) => Ok(r.into_json::<Value>()?),
        Err(ureq::Error::Status(_, r)) => Ok(r.into_json::<Value>().unwrap_or_else(|_| json!({}))),
        Err(e) => Err(http_err(e)),
    }
}

fn post_json(url: &str, body: &Value) -> Result<Value> {
    let req = http().post(url).set("Accept", "application/json");
    match req.send_json(body.clone()) {
        Ok(r) => Ok(r.into_json::<Value>()?),
        Err(ureq::Error::Status(_, r)) => Ok(r.into_json::<Value>().unwrap_or_else(|_| json!({}))),
        Err(e) => Err(http_err(e)),
    }
}

/// Microsoft app (client) ID baked in at build time. Set the `SPIDERFLAYER_MS_CLIENT_ID`
/// environment variable when building (the GitHub workflow does this from a repo variable)
/// so players never have to enter anything. Must be an Azure app approved by Mojang.
pub const DEFAULT_CLIENT_ID: &str = match option_env!("SPIDERFLAYER_MS_CLIENT_ID") {
    Some(v) => v,
    None => "",
};

/// The client ID to use: the user's own override if set, otherwise the built-in one.
pub fn client_id(user_setting: &str) -> String {
    let u = user_setting.trim();
    if u.is_empty() {
        DEFAULT_CLIENT_ID.trim().to_string()
    } else {
        u.to_string()
    }
}

/// A sign-in waiting for the browser to come back to our loopback address.
pub struct PendingLogin {
    listener: TcpListener,
    verifier: String,
    state: String,
    redirect: String,
    client_id: String,
}

fn random_token(bytes: usize) -> Result<String> {
    let mut buf = vec![0u8; bytes];
    getrandom::getrandom(&mut buf).map_err(|e| anyhow!("no secure random source: {e}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

/// Step 1: open a loopback listener and build the Microsoft sign-in URL (PKCE).
/// The caller opens the URL in a browser (or an in-app window) and then calls
/// `ms_complete`.
pub fn ms_prepare(client_id: &str) -> Result<(String, PendingLogin)> {
    let client_id = client_id.trim();
    if client_id.is_empty() {
        bail!("Premium sign-in isn't set up in this build yet. Add a Microsoft app ID (see the README) or use an offline account.");
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let redirect = format!("http://localhost:{port}");
    let verifier = random_token(48)?;
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(Sha256::digest(verifier.as_bytes()));
    let state = random_token(16)?;
    let url = format!(
        "{TENANT}/authorize?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256&prompt=select_account",
        urlencoding::encode(client_id),
        urlencoding::encode(&redirect),
        urlencoding::encode(SCOPE),
        state,
        challenge
    );
    Ok((
        url,
        PendingLogin {
            listener,
            verifier,
            state,
            redirect,
            client_id: client_id.to_string(),
        },
    ))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn respond(stream: &mut TcpStream, status: &str, ok: bool, message: &str) {
    let (title, color) = if ok {
        ("You're signed in", "#3ddc84")
    } else {
        ("Sign-in didn't finish", "#ff3b4a")
    };
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>Spiderflayer Launcher</title>\
<body style=\"margin:0;min-height:100vh;display:grid;place-items:center;background:#000;color:#f7f1f1;font:16px system-ui,sans-serif\">\
<div style=\"text-align:center;padding:32px;max-width:420px\">\
<div style=\"font-size:44px;color:{color}\">&#9679;</div>\
<h1 style=\"margin:8px 0\">{title}</h1>\
<p style=\"color:#c9bcbe\">{}</p></div></body>",
        html_escape(message)
    );
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html.len(),
        html
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}

fn parse_query(q: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let v = v.replace('+', " ");
            let decoded: Option<String> = urlencoding::decode(&v).ok().map(|c| c.into_owned());
            out.insert(k.to_string(), decoded.unwrap_or(v));
        }
    }
    out
}

/// Step 2: wait for the browser to be redirected back, then finish the Xbox/Minecraft chain.
/// `cancel` lets the UI abort the wait; `timeout_secs` bounds it.
pub fn ms_complete(
    paths: &Paths,
    pending: PendingLogin,
    cancel: &AtomicBool,
    timeout_secs: u64,
) -> Result<Account> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let code: String = loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("Sign-in cancelled");
        }
        if Instant::now() > deadline {
            bail!("Sign-in timed out. Try again.");
        }
        match pending.listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = req.lines().next().unwrap_or("");
                let target = line.split_whitespace().nth(1).unwrap_or("");
                let query = target.split_once('?').map(|x| x.1).unwrap_or("");
                let params = parse_query(query);
                if let Some(e) = params.get("error") {
                    let why = params
                        .get("error_description")
                        .cloned()
                        .unwrap_or_else(|| e.clone());
                    respond(&mut stream, "400 Bad Request", false, &why);
                    bail!("Microsoft sign-in failed: {why}");
                }
                match params.get("code") {
                    Some(code) => {
                        if params.get("state") != Some(&pending.state) {
                            respond(&mut stream, "400 Bad Request", false, "This sign-in link is not for this launcher window. Start again from Spiderflayer.");
                            continue;
                        }
                        respond(
                            &mut stream,
                            "200 OK",
                            true,
                            "You can close this tab and go back to Spiderflayer Launcher.",
                        );
                        break code.clone();
                    }
                    None => {
                        // favicon requests and the like
                        respond(&mut stream, "404 Not Found", false, "Nothing here.");
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => return Err(e.into()),
        }
    };

    let r = post_form(
        &format!("{TENANT}/token"),
        &[
            ("client_id", pending.client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", pending.redirect.as_str()),
            ("code_verifier", pending.verifier.as_str()),
            ("scope", SCOPE),
        ],
    )?;
    let access = match r["access_token"].as_str() {
        Some(a) => a.to_string(),
        None => bail!(
            "Microsoft sign-in failed: {}",
            r["error_description"].as_str().unwrap_or("no access token returned")
        ),
    };
    let refresh = r["refresh_token"].as_str().unwrap_or("").to_string();
    let session = mc_login(&access)?;
    store_session(paths, &refresh, session)
}

/// Step 1: ask Microsoft for a code the user types in at microsoft.com/link.
pub fn ms_begin(client_id: &str) -> Result<Value> {
    if client_id.trim().is_empty() {
        bail!("Add your Microsoft app client ID in Settings first (see the README)");
    }
    let r = post_form(
        &format!("{TENANT}/devicecode"),
        &[("client_id", client_id.trim()), ("scope", SCOPE)],
    )?;
    if r["device_code"].is_null() {
        bail!(
            "Microsoft refused the request: {}",
            r["error_description"].as_str().unwrap_or("unknown error")
        );
    }
    Ok(r)
}

struct McSession {
    token: String,
    expires: u64,
    uuid: String,
    name: String,
}

fn mc_login(ms_access: &str) -> Result<McSession> {
    let xbl = post_json(
        "https://user.auth.xboxlive.com/user/authenticate",
        &json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access}")
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )?;
    let xbl_token = xbl["Token"]
        .as_str()
        .ok_or_else(|| anyhow!("Xbox Live sign-in failed"))?
        .to_string();
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .ok_or_else(|| anyhow!("Xbox Live sign-in returned no user hash"))?
        .to_string();

    let xsts = post_json(
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        &json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )?;
    if let Some(code) = xsts["XErr"].as_u64() {
        bail!(match code {
            2148916233 => "This Microsoft account has no Xbox profile. Sign in once at xbox.com to create one.".to_string(),
            2148916235 => "Xbox Live is not available in your country.".to_string(),
            2148916238 => "This is a child account. Add it to a Microsoft family first.".to_string(),
            c => format!("Xbox authorization failed (code {c})"),
        });
    }
    let xsts_token = xsts["Token"]
        .as_str()
        .ok_or_else(|| anyhow!("Xbox authorization returned no token"))?;

    let mc = post_json(
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        &json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }),
    )?;
    let token = mc["access_token"]
        .as_str()
        .ok_or_else(|| {
            let why = mc["errorMessage"]
                .as_str()
                .or_else(|| mc["error"].as_str())
                .unwrap_or("unknown error");
            if why.contains("Invalid app registration") {
                anyhow!("Mojang has not approved this Azure app yet. Request approval at https://aka.ms/AppRegInfo, or play with an offline account in the meantime.")
            } else {
                anyhow!("Minecraft sign-in failed: {why}")
            }
        })?
        .to_string();
    let expires = now_secs() + mc["expires_in"].as_u64().unwrap_or(86_000);

    let profile = match http()
        .get("https://api.minecraftservices.com/minecraft/profile")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
    {
        Ok(r) => r.into_json::<Value>()?,
        Err(ureq::Error::Status(404, _)) => {
            bail!("This Microsoft account does not own Minecraft: Java Edition")
        }
        Err(e) => return Err(http_err(e)),
    };
    let raw = profile["id"].as_str().unwrap_or("");
    if raw.len() != 32 {
        bail!("Minecraft profile has no id");
    }
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &raw[0..8],
        &raw[8..12],
        &raw[12..16],
        &raw[16..20],
        &raw[20..32]
    );
    Ok(McSession {
        token,
        expires,
        uuid,
        name: profile["name"].as_str().unwrap_or("Player").to_string(),
    })
}

fn store_session(paths: &Paths, refresh: &str, s: McSession) -> Result<Account> {
    let mut store = load(paths);
    let acct = Account {
        id: s.uuid.clone(),
        name: s.name,
        kind: "microsoft".into(),
        access_token: s.token,
        refresh_token: refresh.to_string(),
        expires: s.expires,
    };
    store.accounts.retain(|a| a.id != acct.id);
    store.accounts.push(acct.clone());
    store.selected = acct.id.clone();
    save(paths, &store)?;
    Ok(acct)
}

/// Step 2: poll until the user has approved the code, then finish the Xbox/Minecraft chain.
pub fn ms_finish(
    paths: &Paths,
    client_id: &str,
    device_code: &str,
    interval: u64,
    expires_in: u64,
) -> Result<Account> {
    let deadline = now_secs() + expires_in.max(60);
    let mut wait = interval.max(3);
    loop {
        if now_secs() > deadline {
            bail!("The sign-in code expired. Try again.");
        }
        std::thread::sleep(Duration::from_secs(wait));
        let r = post_form(
            &format!("{TENANT}/token"),
            &[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id.trim()),
                ("device_code", device_code),
            ],
        )?;
        if let Some(access) = r["access_token"].as_str() {
            let refresh = r["refresh_token"].as_str().unwrap_or("").to_string();
            let session = mc_login(access)?;
            return store_session(paths, &refresh, session);
        }
        match r["error"].as_str() {
            Some("authorization_pending") => {}
            Some("slow_down") => wait += 5,
            _ => bail!(
                "{}",
                r["error_description"].as_str().unwrap_or("Sign-in was cancelled")
            ),
        }
    }
}

/// Make sure the selected account has a valid token, refreshing Microsoft ones when needed.
pub fn ensure_fresh(paths: &Paths, client_id: &str) -> Result<Account> {
    let store = load(paths);
    let acct = selected(&store).ok_or_else(|| anyhow!("Add an account first (click your avatar)"))?;
    if acct.kind != "microsoft" || acct.expires > now_secs() + 120 {
        return Ok(acct);
    }
    if acct.refresh_token.is_empty() {
        bail!("Your Microsoft session expired. Sign in again.");
    }
    let r = post_form(
        &format!("{TENANT}/token"),
        &[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.trim()),
            ("refresh_token", &acct.refresh_token),
            ("scope", SCOPE),
        ],
    )?;
    let access = r["access_token"]
        .as_str()
        .ok_or_else(|| anyhow!("Your Microsoft session expired. Sign in again."))?;
    let refresh = r["refresh_token"].as_str().unwrap_or(&acct.refresh_token).to_string();
    let session = mc_login(access)?;
    store_session(paths, &refresh, session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_url_has_required_parts() {
        let (url, pending) = ms_prepare("abc-123").unwrap();
        assert!(url.contains("client_id=abc-123"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(&format!("state={}", pending.state)));
        assert!(pending.redirect.starts_with("http://localhost:"));
        assert!(pending.verifier.len() >= 43);
        assert!(ms_prepare("  ").is_err());
    }

    #[test]
    fn query_parsing_decodes() {
        let q = parse_query("code=a%2Fb+c&state=xyz");
        assert_eq!(q.get("code").unwrap(), "a/b c");
        assert_eq!(q.get("state").unwrap(), "xyz");
    }

    #[test]
    fn client_id_prefers_user_override() {
        assert_eq!(client_id(" mine "), "mine");
    }

    #[test]
    fn offline_uuid_is_stable_and_version_3() {
        let a = offline_uuid("Steve");
        assert_eq!(a, offline_uuid("Steve"));
        assert_ne!(a, offline_uuid("Alex"));
        assert_eq!(a.len(), 36);
        assert_eq!(a.as_bytes()[14], b'3');
    }
}
