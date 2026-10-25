//! Instances: one folder per profile, with its own mods, saves and options.

use crate::config::{Paths, Settings};
use crate::util::*;
use crate::{java, loaders, mojang};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub mc_version: String,
    /// vanilla | fabric | quilt | forge | neoforge
    pub loader: String,
    pub loader_version: String,
    /// Launchable version id once installed (e.g. `fabric-loader-0.16.5-1.21`).
    pub version_id: String,
    /// 0 = use the global default.
    pub memory_mb: u32,
    pub java_args: String,
    pub created: u64,
    pub last_played: u64,
}

fn file(paths: &Paths, id: &str) -> Result<PathBuf> {
    Ok(paths.instance_dir(id)?.join("instance.json"))
}

pub fn list(paths: &Paths) -> Vec<Instance> {
    let mut out: Vec<Instance> = fs::read_dir(paths.instances())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| fs::read(e.path().join("instance.json")).ok())
                .filter_map(|b| serde_json::from_slice::<Instance>(&b).ok())
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.last_played.max(b.created).cmp(&a.last_played.max(a.created)));
    out
}

pub fn get(paths: &Paths, id: &str) -> Result<Instance> {
    let bytes = fs::read(file(paths, id)?)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn save(paths: &Paths, inst: &Instance) -> Result<()> {
    write_json_file(&file(paths, &inst.id)?, inst)
}

fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "instance".to_string()
    } else {
        s.chars().take(40).collect()
    }
}

pub fn create(
    paths: &Paths,
    name: &str,
    mc: &str,
    loader: &str,
    loader_version: &str,
) -> Result<Instance> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Give the instance a name");
    }
    if !loaders::LOADERS.contains(&loader) {
        bail!("unknown loader {loader}");
    }
    safe_file_name(mc)?;
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while paths.instance_dir(&id)?.exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    let inst = Instance {
        id,
        name: name.to_string(),
        mc_version: mc.to_string(),
        loader: loader.to_string(),
        loader_version: loader_version.to_string(),
        created: now_secs(),
        ..Default::default()
    };
    fs::create_dir_all(paths.game_dir(&inst.id)?)?;
    save(paths, &inst)?;
    Ok(inst)
}

pub fn delete(paths: &Paths, id: &str) -> Result<()> {
    let dir = paths.instance_dir(id)?;
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

// ---- content (mods, resource packs, shaders) -----------------------------------------

#[derive(Serialize, Clone, Debug)]
pub struct ContentFile {
    pub name: String,
    pub enabled: bool,
    pub size: u64,
}

fn content_dir(paths: &Paths, id: &str, kind: &str) -> Result<PathBuf> {
    match kind {
        "mods" | "resourcepacks" | "shaderpacks" => Ok(paths.game_dir(id)?.join(kind)),
        _ => bail!("unknown content type {kind}"),
    }
}

pub fn content(paths: &Paths, id: &str, kind: &str) -> Result<Vec<ContentFile>> {
    let dir = content_dir(paths, id, kind)?;
    let mut out: Vec<ContentFile> = fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().is_file())
                .filter_map(|e| {
                    let name = e.file_name().into_string().ok()?;
                    let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                    let enabled = !name.ends_with(".disabled");
                    Some(ContentFile { name, enabled, size })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

pub fn set_enabled(paths: &Paths, id: &str, kind: &str, name: &str, enabled: bool) -> Result<()> {
    let dir = content_dir(paths, id, kind)?;
    let name = safe_file_name(name)?;
    let (from, to) = if enabled {
        match name.strip_suffix(".disabled") {
            Some(n) => (name.clone(), n.to_string()),
            None => return Ok(()),
        }
    } else if name.ends_with(".disabled") {
        return Ok(());
    } else {
        (name.clone(), format!("{name}.disabled"))
    };
    fs::rename(dir.join(from), dir.join(to))?;
    Ok(())
}

pub fn remove(paths: &Paths, id: &str, kind: &str, name: &str) -> Result<()> {
    let dir = content_dir(paths, id, kind)?;
    fs::remove_file(dir.join(safe_file_name(name)?))?;
    Ok(())
}

// ---- preparing a launch --------------------------------------------------------------

/// Install everything the instance needs and return the Java executable to run it with.
pub fn prepare(
    paths: &Paths,
    inst: &mut Instance,
    settings: &Settings,
    rep: Reporter,
) -> Result<PathBuf> {
    mojang::ensure_vanilla_json(paths, &inst.mc_version)?;
    let vanilla = mojang::resolve(paths, &inst.mc_version)?;
    let java = java::resolve_java(paths, &settings.java_path, &vanilla, rep)?;
    mojang::install_version(paths, &inst.mc_version, rep)?;

    if inst.loader == "vanilla" {
        inst.version_id = inst.mc_version.clone();
    } else {
        let have = !inst.version_id.is_empty()
            && paths.version_json(&inst.version_id).exists();
        if !have {
            let lv = if inst.loader_version.is_empty() {
                loaders::versions(&inst.loader, &inst.mc_version)?
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        anyhow::anyhow!("no {} build for Minecraft {}", inst.loader, inst.mc_version)
                    })?
            } else {
                inst.loader_version.clone()
            };
            inst.version_id =
                loaders::install(paths, &inst.loader, &inst.mc_version, &lv, &java, rep)?;
            inst.loader_version = lv;
        }
        mojang::install_version(paths, &inst.version_id, rep)?;
    }
    Ok(java)
}
