//! Automatic install of Horizon for the "Rust" instance.
//!
//! 1. Pick the newest Horizon game release on GitHub that has a build for this system.
//! 2. Download it and install it only if its SHA-256 matches the release's SHA256SUMS and that list
//!    is signed by a key built into this launcher (the same check Horizon's own launcher does).
//! 3. Prepare it: Horizon builds its game files from the player's own Minecraft. This launcher
//!    installs Minecraft 26.3 itself (a Microsoft account that owns the game is required), downloads
//!    Mojang's Java runtime and server jar, then runs Horizon's own `horizon-prepare`.
//!
//! Nothing here copies Horizon's code: it is Horizon's own release that gets installed.

use crate::config::Paths;
use crate::horizon::{self, Install};
use crate::util::*;
use crate::{java, mojang};
use anyhow::{anyhow, bail, Context, Result};
use horizon_release_sign as sign;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const REPO: &str = "horizon-rust/horizon";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const ASSET: Option<&str> = Some("horizon-linux-x86_64.tar.gz");
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const ASSET: Option<&str> = Some("horizon-macos-arm64.zip");
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const ASSET: Option<&str> = Some("horizon-windows-x64.zip");
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
    all(target_os = "windows", target_arch = "x86_64")
)))]
const ASSET: Option<&str> = None;

#[cfg(windows)]
const PREPARE: &str = "horizon-prepare.exe";
#[cfg(not(windows))]
const PREPARE: &str = "horizon-prepare";

/// Where this launcher keeps the Horizon versions it installed itself.
pub fn own_versions(paths: &Paths) -> PathBuf {
    paths.root.join("horizon").join("versions")
}

/// A game release tag looks like `v2026.10.13`. (Launcher releases are `launcher@…`.)
pub fn is_game_tag(tag: &str) -> bool {
    tag.strip_prefix('v')
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// The newest published game release in GitHub's release list that has a build for `asset`.
pub fn pick_release(list: &Value, asset: &str) -> Option<String> {
    list.as_array()?
        .iter()
        .filter(|r| {
            !r["draft"].as_bool().unwrap_or(false) && !r["prerelease"].as_bool().unwrap_or(false)
        })
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?;
            let has = r["assets"]
                .as_array()?
                .iter()
                .any(|a| a["name"].as_str() == Some(asset));
            (is_game_tag(tag) && has).then(|| tag.to_string())
        })
        .next()
}

fn latest_tag() -> Result<String> {
    let asset = ASSET.ok_or_else(|| anyhow!("Horizon has no build for this system (it supports Windows x64, Linux x86-64 and Apple Silicon Macs)."))?;
    let list: Value = http()
        .get(&format!(
            "https://api.github.com/repos/{REPO}/releases?per_page=100"
        ))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(http_err)
        .and_then(|r| r.into_json().context("unreadable release list"))?;
    pick_release(&list, asset)
        .ok_or_else(|| anyhow!("No Horizon release for this system was found."))
}

fn asset_url(tag: &str, name: &str) -> String {
    format!("https://github.com/{REPO}/releases/download/{tag}/{name}")
}

/// A small release file, whole. None when the release doesn't have it.
fn fetch_small(url: &str) -> Result<Option<Vec<u8>>> {
    match http().get(url).call() {
        Ok(r) => {
            let mut body = Vec::new();
            r.into_reader().take(1 << 20).read_to_end(&mut body)?;
            Ok(Some(body))
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(e) => Err(http_err(e)),
    }
}

/// The SHA-256 this release's archive must have: signed by a built-in key, or refused.
fn signed_sha256(tag: &str, asset: &str) -> Result<String> {
    if let Some(hex) = sign::legacy_digest(tag, asset) {
        return Ok(hex.to_string());
    }
    let sums = fetch_small(&asset_url(tag, sign::SUMS))?;
    let signature = fetch_small(&asset_url(tag, sign::SIGNATURE))?
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    sign::signed_digest(
        tag,
        asset,
        sums.as_deref(),
        signature.as_deref(),
        &sign::trusted_keys(),
    )
    .map_err(|e| anyhow!("{e}"))
}

fn download_checked(url: &str, expected: &str, to: &Path, rep: Reporter) -> Result<()> {
    let resp = http().get(url).call().map_err(http_err)?;
    let total: u64 = resp
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut reader = resp.into_reader();
    let mut file = fs::File::create(to)?;
    let mut hash = sign::Sha256::default();
    let mut buf = vec![0u8; 64 * 1024];
    let (mut done, mut last) = (0u64, 0u64);
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        file.write_all(&buf[..n])?;
        done += n as u64;
        if done - last >= 512 * 1024 {
            last = done;
            rep(Progress::new("horizon", done, total, "Downloading Horizon"));
        }
    }
    file.flush()?;
    let name = url.rsplit('/').next().unwrap_or(url);
    sign::check(name, expected, &hash.hex()).map_err(|e| {
        let _ = fs::remove_file(to);
        anyhow!("{e}")
    })
}

fn unpack(archive: &Path, name: &str, to: &Path) -> Result<()> {
    let file = fs::File::open(archive)?;
    if name.ends_with(".tar.gz") {
        tar::Archive::new(flate2::read::GzDecoder::new(file)).unpack(to)?;
    } else {
        zip::ZipArchive::new(file)?.extract(to)?;
    }
    Ok(())
}

#[cfg(unix)]
fn make_executable(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o755));
}
#[cfg(not(unix))]
fn make_executable(_: &Path) {}

/// Downloads, verifies and unpacks release `tag` into this launcher's Horizon folder.
pub fn install_release(paths: &Paths, tag: &str, rep: Reporter) -> Result<PathBuf> {
    if !is_game_tag(tag) {
        bail!("{tag} is not a Horizon game release");
    }
    let asset = ASSET.ok_or_else(|| anyhow!("Horizon has no build for this system."))?;
    rep(Progress::msg(
        "horizon",
        "Checking Horizon's signed release",
    ));
    let expected = signed_sha256(tag, asset)?;

    let versions = own_versions(paths);
    let partial = versions.join(format!(".{tag}.partial"));
    let _ = fs::remove_dir_all(&partial);
    fs::create_dir_all(&partial)?;
    let archive = partial.join("download");
    download_checked(&asset_url(tag, asset), &expected, &archive, rep)?;
    rep(Progress::msg("horizon", "Unpacking Horizon"));
    let unpacked = partial.join("unpacked");
    unpack(&archive, asset, &unpacked)?;

    // The archive holds one top folder; the version folder is its contents.
    let entries: Vec<_> = fs::read_dir(&unpacked)?.flatten().collect();
    let top = match entries.as_slice() {
        [only] if only.path().is_dir() => only.path(),
        _ => unpacked.clone(),
    };
    let client = if cfg!(windows) {
        "horizon-client.exe"
    } else {
        "horizon-client"
    };
    if !top.join(client).is_file() {
        bail!("The download has no {client}");
    }
    make_executable(&top.join(client));
    make_executable(&top.join(PREPARE));
    let target = versions.join(tag);
    let _ = fs::remove_dir_all(&target);
    fs::rename(&top, &target)?;
    fs::write(target.join(".installed"), "")?;
    let _ = fs::remove_dir_all(&partial);
    Ok(target)
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Prepares an installed version from Minecraft installed by this launcher.
pub fn prepare(paths: &Paths, dir: &Path, rep: Reporter) -> Result<()> {
    let lock = read_json(&dir.join("research-lock.json"));
    let version = lock
        .as_ref()
        .and_then(|l| l["minecraft"]["version"].as_str())
        .unwrap_or("26.3")
        .to_string();
    safe_file_name(&version)?;

    // Minecraft itself (client jar, libraries, assets) in the standard layout under our data folder.
    mojang::ensure_vanilla_json(paths, &version)?;
    let manifest = mojang::read_version(paths, &version)?;
    mojang::install_version(paths, &version, rep)?;

    // Mojang's own Java for that version.
    let java_exe = java::ensure_runtime(paths, &java::runtime_component(&manifest), rep)?;
    let java_home = java_exe
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| anyhow!("unexpected Java layout"))?
        .to_path_buf();

    // The server jar, from the same version file (it is what Horizon's data generator needs).
    let server = &manifest["downloads"]["server"];
    let url = server["url"]
        .as_str()
        .ok_or_else(|| anyhow!("Minecraft {version} has no server download"))?;
    let jar = paths
        .root
        .join("horizon")
        .join("server-jars")
        .join(format!("{version}.jar"));
    rep(Progress::msg(
        "horizon",
        "Downloading the server jar from Mojang",
    ));
    download(url, &jar, server["sha1"].as_str())?;

    let preparer = dir.join(PREPARE);
    if !preparer.is_file() {
        bail!("This Horizon version has no {PREPARE}. Use a newer one.");
    }
    run_preparer(
        Command::new(&preparer)
            .current_dir(dir)
            .arg("--root")
            .arg(dir)
            .args(["--prepare", "--server-jar"])
            .arg(&jar)
            .arg("--minecraft-dir")
            .arg(&paths.root),
        &java_home,
        rep,
    )?;
    // Sounds are optional: a failure only means a silent game.
    let _ = run_preparer(
        Command::new(&preparer)
            .current_dir(dir)
            .arg("sounds")
            .arg("--root")
            .arg(dir)
            .arg("--minecraft-dir")
            .arg(&paths.root),
        &java_home,
        rep,
    );
    Ok(())
}

fn run_preparer(cmd: &mut Command, java_home: &Path, rep: Reporter) -> Result<()> {
    let mut path = vec![java_home.join("bin")];
    path.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    cmd.env("JAVA_HOME", java_home)
        .env("PATH", std::env::join_paths(path)?)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(cmd);
    let mut child = cmd.spawn().context("could not start horizon-prepare")?;
    let stderr = child.stderr.take().unwrap();
    let err_thread = std::thread::spawn(move || {
        BufReader::new(stderr)
            .lines()
            .map_while(Result::ok)
            .collect::<Vec<_>>()
    });
    for line in BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map_while(Result::ok)
    {
        let line = line.trim();
        if !line.is_empty() {
            rep(Progress::msg(
                "horizon",
                &format!("Preparing Horizon: {line}"),
            ));
        }
    }
    let status = child.wait()?;
    let errs = err_thread.join().unwrap_or_default();
    if !status.success() {
        let tail: Vec<_> = errs.iter().rev().take(4).rev().cloned().collect();
        bail!("Preparing Horizon failed: {}", tail.join(" | "));
    }
    Ok(())
}

/// Makes sure a prepared Horizon exists, installing and preparing it if needed.
/// Reuses a Horizon already installed by Horizon's own launcher.
pub fn ensure(paths: &Paths, rep: Reporter) -> Result<Install> {
    if let Ok(found) = horizon::find(paths) {
        return Ok(found);
    }
    let versions = own_versions(paths);
    let dir = match horizon::newest_installed(&versions) {
        Some(dir) => dir,
        None => {
            let tag = latest_tag()?;
            install_release(paths, &tag, rep)?
        }
    };
    prepare(paths, &dir, rep)?;
    rep(Progress::msg("horizon", "Horizon is ready"));
    horizon::find(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn game_tags() {
        assert!(is_game_tag("v2026.10.13"));
        assert!(!is_game_tag("launcher@1.2.3"));
        assert!(!is_game_tag("nightly"));
    }

    #[test]
    fn picks_newest_published_release_with_a_build() {
        let list = json!([
            {"tag_name": "launcher@2.0.0", "assets": [{"name": "horizon-windows-x64.zip"}]},
            {"tag_name": "v2026.10.15", "draft": true, "assets": [{"name": "horizon-windows-x64.zip"}]},
            {"tag_name": "v2026.10.14", "prerelease": true, "assets": [{"name": "horizon-windows-x64.zip"}]},
            {"tag_name": "v2026.10.13", "assets": [{"name": "horizon-linux-x86_64.tar.gz"}]},
            {"tag_name": "v2026.10.12", "assets": [{"name": "horizon-windows-x64.zip"}]}
        ]);
        assert_eq!(
            pick_release(&list, "horizon-windows-x64.zip").as_deref(),
            Some("v2026.10.12")
        );
        assert_eq!(
            pick_release(&list, "horizon-linux-x86_64.tar.gz").as_deref(),
            Some("v2026.10.13")
        );
        assert_eq!(pick_release(&list, "nothing.zip"), None);
        assert_eq!(pick_release(&json!({}), "x"), None);
    }

    #[test]
    fn built_in_release_key_is_present() {
        assert!(!sign::trusted_keys().is_empty());
    }

    #[test]
    fn unsigned_unknown_releases_are_refused_offline_shape() {
        // Without checksums (None, None) a non-legacy release can never pass.
        assert!(
            sign::signed_digest("v2099.1.1", "a.zip", None, None, &sign::trusted_keys()).is_err()
        );
    }
}
