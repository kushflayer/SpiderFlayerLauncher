//! "Rust" instances run Horizon (https://github.com/horizon-rust/horizon, MIT), an unofficial Rust
//! reimplementation of Minecraft: Java Edition, instead of Java Minecraft. It is fast but cannot
//! run Java mods.
//!
//! Spiderflayer installs Horizon's own signed release by itself (horizon_install.rs), or reuses
//! the copy Horizon's own launcher installed and prepared. The hand-over of the sign-in follows Horizon's documented
//! `--session-file` contract: a short-lived, owner-only JSON file the game reads once.

use crate::config::Paths;
use crate::instance::{self, Instance};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const LOADER: &str = "horizon";
pub const DEFAULT_ID: &str = "rust";
const SEEDED: &str = ".rust-seeded";
const MARK: &str = ".installed";
const MANIFEST: &str = "horizon-resources.json";
const GET_IT: &str = "Horizon is not installed yet.";

#[cfg(windows)]
const CLIENT: &str = "horizon-client.exe";
#[cfg(not(windows))]
const CLIENT: &str = "horizon-client";

/// An installed, prepared Horizon.
#[derive(Debug, Clone, PartialEq)]
pub struct Install {
    pub tag: String,
    pub client: PathBuf,
    pub resources: PathBuf,
}

/// Horizon's launcher data folder (the same override variable the launcher itself honours).
pub fn launcher_home() -> PathBuf {
    if let Some(h) = std::env::var_os("HORIZON_LAUNCHER_HOME").filter(|h| !h.is_empty()) {
        return PathBuf::from(h);
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("horizon-launcher")
}

/// The folder the game runs in (worlds, options).
pub fn game_home() -> PathBuf {
    if let Some(h) = std::env::var_os("HORIZON_HOME").filter(|h| !h.is_empty()) {
        return PathBuf::from(h);
    }
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("horizon")
}

/// Orders version tags like `v2026.10.13` or `v0.2.92` by their numbers, not as text.
fn tag_key(tag: &str) -> Vec<u64> {
    tag.split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().unwrap_or(0))
        .collect()
}

/// The newest installed and prepared version under `versions`.
pub fn find_in(versions: &Path) -> Result<Install> {
    let mut tags: Vec<String> = fs::read_dir(versions)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().join(MARK).exists())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    if tags.is_empty() {
        bail!("{GET_IT}");
    }
    tags.sort_by_key(|t| std::cmp::Reverse(tag_key(t)));
    for tag in &tags {
        let dir = versions.join(tag);
        let client = dir.join(CLIENT);
        if !client.is_file() {
            continue;
        }
        let base = dir.join(".local").join("horizon-client");
        let resources = fs::read_dir(&base)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.join(MANIFEST).is_file());
        if let Some(resources) = resources {
            return Ok(Install {
                tag: tag.clone(),
                client,
                resources,
            });
        }
    }
    bail!("Horizon is installed but not prepared yet. Open the Horizon launcher and press Prepare once. It needs your own Minecraft: Java Edition 26.3.")
}

/// The newest installed folder (prepared or not) under `versions`.
pub fn newest_installed(versions: &Path) -> Option<PathBuf> {
    let mut tags: Vec<String> = fs::read_dir(versions)
        .ok()?
        .flatten()
        .filter(|e| e.path().join(MARK).exists() && e.path().join(CLIENT).is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    tags.sort_by_key(|t| std::cmp::Reverse(tag_key(t)));
    tags.first().map(|t| versions.join(t))
}

/// A prepared Horizon: the one this launcher installed, else the one Horizon's own launcher did.
pub fn find(paths: &Paths) -> Result<Install> {
    find_in(&crate::horizon_install::own_versions(paths))
        .or_else(|_| find_in(&launcher_home().join("versions")))
}

/// Rust (Horizon) is for accounts that own the game, like Horizon's own launcher.
pub fn require_account(account: &crate::auth::Account) -> Result<()> {
    if account.kind != "microsoft" {
        bail!("Rust (Horizon) needs a Microsoft account that owns Minecraft: Java Edition, like Horizon's own launcher. Sign in with Microsoft from your account menu.");
    }
    Ok(())
}

/// Writes the sign-in hand-over file (owner-only, read once by the game, refused after 2 minutes).
pub fn write_session(dir: &Path, name: &str, uuid: &str, token: &str) -> Result<PathBuf> {
    fs::create_dir_all(dir)?;
    // Sweep session files the game never picked up (it refuses them after two minutes anyway).
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|d| d.as_secs() > 600);
        if n.starts_with("session-") && n.ends_with(".json") && old {
            let _ = fs::remove_file(e.path());
        }
    }
    let mut rnd = [0u8; 12];
    getrandom::getrandom(&mut rnd).map_err(|e| anyhow!("{e}"))?;
    let file = dir.join(format!(
        "session-{}.json",
        rnd.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts
        .open(&file)
        .context("could not write the session file")?;
    let body = json!({ "name": name, "uuid": uuid, "access_token": token, "user_type": "msa" });
    std::io::Write::write_all(&mut f, body.to_string().as_bytes())?;
    Ok(file)
}

/// The command that starts Horizon for `account` (a Microsoft account that owns the game).
pub fn command(
    install: &Install,
    account: &crate::auth::Account,
    extra_args: &str,
) -> Result<Command> {
    require_account(account)?;
    let home = game_home();
    fs::create_dir_all(&home)?;
    let session = write_session(&home, &account.name, &account.id, &account.access_token)?;
    let mut cmd = Command::new(&install.client);
    cmd.current_dir(&home)
        .arg("--resources")
        .arg(&install.resources)
        .arg("--session-file")
        .arg(session)
        .args(extra_args.split_whitespace())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::util::hide_window(&mut cmd);
    Ok(cmd)
}

/// Creates the default "Rust" instance once. If the player deletes it, it stays deleted.
pub fn ensure_default(paths: &Paths) -> Result<()> {
    let marker = paths.root.join(SEEDED);
    if marker.exists() {
        return Ok(());
    }
    if !paths.instance_dir(DEFAULT_ID)?.exists() {
        let inst = Instance {
            id: DEFAULT_ID.into(),
            name: "Rust".into(),
            mc_version: "26.3".into(),
            loader: LOADER.into(),
            created: crate::util::now_secs(),
            ..Default::default()
        };
        fs::create_dir_all(paths.game_dir(&inst.id)?)?;
        instance::save(paths, &inst)?;
    }
    fs::write(marker, "")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "x").unwrap();
    }
    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sf-hz-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn tags_sort_numerically() {
        assert!(tag_key("v2026.10.13") > tag_key("v2026.9.30"));
        assert!(tag_key("v0.2.92") > tag_key("v0.2.9"));
    }

    #[test]
    fn finds_newest_prepared_version() {
        let v = tmp("find");
        for (tag, prepared) in [("v0.2.9", true), ("v0.2.92", true), ("v0.2.93", false)] {
            touch(&v.join(tag).join(MARK));
            touch(&v.join(tag).join(CLIENT));
            if prepared {
                touch(
                    &v.join(tag)
                        .join(".local/horizon-client/26.3")
                        .join(MANIFEST),
                );
            }
        }
        let i = find_in(&v).unwrap();
        assert_eq!(i.tag, "v0.2.92");
        assert!(i.resources.ends_with("26.3"));
        let _ = fs::remove_dir_all(&v);
    }

    #[test]
    fn explains_when_missing() {
        let v = tmp("none");
        assert!(find_in(&v.join("versions"))
            .unwrap_err()
            .to_string()
            .contains("playhorizon.dev"));
        touch(&v.join("v1").join(MARK));
        touch(&v.join("v1").join(CLIENT));
        assert!(find_in(&v).unwrap_err().to_string().contains("Prepare"));
        let _ = fs::remove_dir_all(&v);
    }

    #[test]
    fn session_file_has_the_documented_fields() {
        let d = tmp("sess");
        let f = write_session(&d, "Steve", "069a79f444e94726a5befca90e38aaf5", "tok").unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(f).unwrap()).unwrap();
        assert_eq!(v["name"], "Steve");
        assert_eq!(v["user_type"], "msa");
        assert_eq!(v["access_token"], "tok");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn offline_accounts_are_refused() {
        let i = Install {
            tag: "v1".into(),
            client: "x".into(),
            resources: "y".into(),
        };
        let a = crate::auth::Account {
            kind: "offline".into(),
            ..Default::default()
        };
        assert!(command(&i, &a, "")
            .unwrap_err()
            .to_string()
            .contains("Microsoft"));
    }

    #[test]
    fn default_instance_is_seeded_once() {
        let d = tmp("seed");
        let paths = Paths { root: d.clone() };
        ensure_default(&paths).unwrap();
        assert!(paths
            .instance_dir(DEFAULT_ID)
            .unwrap()
            .join("instance.json")
            .exists());
        fs::remove_dir_all(paths.instance_dir(DEFAULT_ID).unwrap()).unwrap();
        ensure_default(&paths).unwrap();
        assert!(!paths.instance_dir(DEFAULT_ID).unwrap().exists());
        let _ = fs::remove_dir_all(&d);
    }
}
