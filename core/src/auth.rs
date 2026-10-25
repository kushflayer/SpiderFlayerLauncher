//! Accounts. Offline ("cracked") accounts are just a name plus a deterministic UUID.
//! Microsoft accounts use the OAuth device-code flow, so no embedded browser or local
//! redirect server is needed and the launcher stays tiny.

use crate::config::Paths;
use crate::util::*;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

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
    fn offline_uuid_is_stable_and_version_3() {
        let a = offline_uuid("Steve");
        assert_eq!(a, offline_uuid("Steve"));
        assert_ne!(a, offline_uuid("Alex"));
        assert_eq!(a.len(), 36);
        assert_eq!(a.as_bytes()[14], b'3');
    }
}
