//! Modrinth modpacks (.mrpack): create an instance, fetch every file, apply overrides.

use crate::config::Paths;
use crate::instance::{self, Instance};
use crate::util::*;
use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn install_mrpack(paths: &Paths, file: &Path, rep: Reporter) -> Result<Instance> {
    rep(Progress::msg("modpack", "Reading modpack"));
    let mut zip = zip::ZipArchive::new(fs::File::open(file)?)?;
    let index: Value = {
        let mut entry = zip
            .by_name("modrinth.index.json")
            .map_err(|_| anyhow!("This is not a Modrinth modpack (.mrpack)"))?;
        let mut s = String::new();
        entry.read_to_string(&mut s)?;
        serde_json::from_str(&s)?
    };

    let deps = &index["dependencies"];
    let mc = deps["minecraft"]
        .as_str()
        .ok_or_else(|| anyhow!("modpack does not name a Minecraft version"))?;
    let (loader, lv) = [
        ("fabric", "fabric-loader"),
        ("quilt", "quilt-loader"),
        ("neoforge", "neoforge"),
        ("forge", "forge"),
    ]
    .iter()
    .find_map(|(l, key)| deps[*key].as_str().map(|v| (*l, v.to_string())))
    .unwrap_or(("vanilla", String::new()));

    let name = index["name"].as_str().unwrap_or("Modpack");
    let inst = instance::create(paths, name, mc, loader, &lv)?;
    let game_dir = paths.game_dir(&inst.id)?;

    let files: Vec<Value> = index["files"].as_array().cloned().unwrap_or_default();
    let wanted: Vec<&Value> = files
        .iter()
        .filter(|f| f["env"]["client"].as_str() != Some("unsupported"))
        .collect();
    let total = wanted.len() as u64;
    let done = AtomicU64::new(0);
    let result = par_each(&wanted, 8, |f| {
        let rel = safe_rel(f["path"].as_str().unwrap_or(""))?;
        let url = f["downloads"][0]
            .as_str()
            .ok_or_else(|| anyhow!("pack file has no download url"))?;
        download(url, &game_dir.join(rel), f["hashes"]["sha1"].as_str())?;
        let d = done.fetch_add(1, Ordering::Relaxed) + 1;
        rep(Progress::new("modpack", d, total, "Downloading modpack files"));
        Ok(())
    });
    if let Err(e) = result {
        let _ = instance::delete(paths, &inst.id);
        return Err(e);
    }

    // overrides/ first, client-overrides/ on top.
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let raw = entry.name().to_string();
            let rest = match raw.strip_prefix(prefix) {
                Some(r) if !r.is_empty() => r.to_string(),
                _ => continue,
            };
            if entry.is_dir() {
                continue;
            }
            let target = game_dir.join(safe_rel(&rest)?);
            if let Some(p) = target.parent() {
                fs::create_dir_all(p)?;
            }
            let mut out = fs::File::create(target)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(inst)
}

/// Install the newest version of a Modrinth modpack project as a new instance.
pub fn install_from_modrinth(paths: &Paths, project: &str, rep: Reporter) -> Result<Instance> {
    rep(Progress::msg("modpack", "Finding the latest modpack version"));
    let versions: Vec<Value> = get_json(&format!(
        "https://api.modrinth.com/v2/project/{}/version",
        urlencoding::encode(project)
    ))?;
    let ver = versions
        .first()
        .ok_or_else(|| anyhow!("this modpack has no versions"))?;
    let files = ver["files"].as_array().cloned().unwrap_or_default();
    let file = files
        .iter()
        .find(|f| f["primary"].as_bool().unwrap_or(false))
        .or_else(|| files.first())
        .ok_or_else(|| anyhow!("this modpack version has no files"))?;
    let url = file["url"].as_str().ok_or_else(|| anyhow!("no download url"))?;
    let name = safe_file_name(file["filename"].as_str().unwrap_or("pack.mrpack"))?;
    if !name.ends_with(".mrpack") {
        bail!("this modpack is not in the .mrpack format");
    }
    let tmp = paths.tmp().join(&name);
    download(url, &tmp, file["hashes"]["sha1"].as_str())?;
    let result = install_mrpack(paths, &tmp, rep);
    let _ = fs::remove_file(&tmp);
    result
}
