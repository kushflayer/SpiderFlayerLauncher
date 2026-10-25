//! Java runtime discovery. Prefers Mojang's own bundled runtimes (downloaded on demand,
//! so the user never has to install Java), falls back to whatever `java` is on PATH.

use crate::config::Paths;
use crate::util::*;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RUNTIME_ALL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

fn platform() -> Option<&'static str> {
    match (os_name(), current_arch()) {
        ("windows", "x86_64") => Some("windows-x64"),
        ("windows", "arm64") => Some("windows-arm64"),
        ("windows", "x86") => Some("windows-x86"),
        ("linux", "x86_64") => Some("linux"),
        ("linux", "x86") => Some("linux-i386"),
        ("osx", "x86_64") => Some("mac-os"),
        ("osx", "arm64") => Some("mac-os-arm64"),
        _ => None,
    }
}

fn find_java(dir: &Path) -> Option<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    [
        dir.join("bin").join(exe),
        dir.join("jre.bundle/Contents/Home/bin").join(exe),
    ]
    .into_iter()
    .find(|p| p.exists())
}

pub fn runtime_component(version: &Value) -> String {
    version["javaVersion"]["component"]
        .as_str()
        .unwrap_or("jre-legacy")
        .to_string()
}

pub fn ensure_runtime(paths: &Paths, component: &str, rep: Reporter) -> Result<PathBuf> {
    safe_file_name(component)?;
    let dir = paths.runtime().join(component);
    if let Some(j) = find_java(&dir) {
        return Ok(j);
    }
    let plat = platform().ok_or_else(|| anyhow!("no bundled Java for this platform"))?;
    rep(Progress::msg("java", "Downloading Java runtime"));
    let all: Value = get_json(RUNTIME_ALL)?;
    let manifest_url = all[plat][component][0]["manifest"]["url"]
        .as_str()
        .ok_or_else(|| anyhow!("Java runtime {component} is not available for {plat}"))?
        .to_string();
    let manifest: Value = get_json(&manifest_url)?;
    let files = manifest["files"]
        .as_object()
        .ok_or_else(|| anyhow!("bad Java runtime manifest"))?;

    let mut jobs: Vec<(PathBuf, String, String, bool)> = Vec::new();
    let mut links: Vec<(PathBuf, String)> = Vec::new();
    for (rel, f) in files {
        let target = dir.join(safe_rel(rel)?);
        match f["type"].as_str() {
            Some("directory") => fs::create_dir_all(&target)?,
            Some("file") => {
                let raw = &f["downloads"]["raw"];
                if let (Some(url), Some(sha)) = (raw["url"].as_str(), raw["sha1"].as_str()) {
                    jobs.push((
                        target,
                        url.to_string(),
                        sha.to_string(),
                        f["executable"].as_bool().unwrap_or(false),
                    ));
                }
            }
            Some("link") => {
                if let Some(t) = f["target"].as_str() {
                    links.push((target, t.to_string()));
                }
            }
            _ => {}
        }
    }

    let total = jobs.len() as u64;
    let done = std::sync::atomic::AtomicU64::new(0);
    par_each(&jobs, 8, |(path, url, sha, exec)| {
        download(url, path, Some(sha))?;
        #[cfg(unix)]
        if *exec {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
        #[cfg(not(unix))]
        let _ = exec;
        let d = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if d % 10 == 0 || d == total {
            rep(Progress::new("java", d, total, "Downloading Java runtime"));
        }
        Ok(())
    })?;

    #[cfg(unix)]
    for (link, target) in &links {
        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent)?;
        }
        let _ = fs::remove_file(link);
        std::os::unix::fs::symlink(target, link)?;
    }
    #[cfg(not(unix))]
    let _ = &links;

    find_java(&dir).ok_or_else(|| anyhow!("Java runtime downloaded but java executable not found"))
}

pub fn system_java() -> Option<PathBuf> {
    let mut cmd = Command::new("java");
    cmd.arg("-version");
    hide_window(&mut cmd);
    match cmd.output() {
        Ok(o) if o.status.success() => Some(PathBuf::from("java")),
        _ => None,
    }
}

/// Pick the Java to run a version with: user override, then Mojang runtime, then system Java.
pub fn resolve_java(
    paths: &Paths,
    user_path: &str,
    version: &Value,
    rep: Reporter,
) -> Result<PathBuf> {
    if !user_path.trim().is_empty() {
        return Ok(PathBuf::from(user_path.trim()));
    }
    match ensure_runtime(paths, &runtime_component(version), rep) {
        Ok(p) => Ok(p),
        Err(e) => system_java().ok_or_else(|| {
            anyhow!("could not get a Java runtime ({e:#}) and no Java was found on this computer")
        }),
    }
}
