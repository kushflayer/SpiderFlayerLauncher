//! Mod loader support: Fabric, Quilt (profile JSON) and Forge, NeoForge (official installers).

use crate::config::Paths;
use crate::util::*;
use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const LOADERS: [&str; 5] = ["vanilla", "fabric", "quilt", "forge", "neoforge"];

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

fn text(url: &str) -> Result<String> {
    Ok(http().get(url).call().map_err(http_err)?.into_string()?)
}

fn xml_versions(xml: &str) -> Vec<String> {
    xml.split("<version>")
        .skip(1)
        .filter_map(|s| s.split("</version>").next())
        .map(String::from)
        .collect()
}

fn neo_prefix(mc: &str) -> Option<String> {
    let p: Vec<&str> = mc.split('.').collect();
    if p.len() < 2 || p[0] != "1" {
        return None;
    }
    let minor = p.get(2).copied().unwrap_or("0");
    Some(format!("{}.{}.", p[1], minor))
}

/// Loader versions available for a Minecraft version, newest first.
pub fn versions(loader: &str, mc: &str) -> Result<Vec<String>> {
    match loader {
        "fabric" | "quilt" => {
            let base = if loader == "fabric" {
                "https://meta.fabricmc.net/v2/versions/loader"
            } else {
                "https://meta.quiltmc.org/v3/versions/loader"
            };
            let list: Vec<Value> = get_json(&format!("{base}/{}", enc(mc)))?;
            Ok(list
                .iter()
                .filter_map(|x| x["loader"]["version"].as_str().map(String::from))
                .collect())
        }
        "forge" => {
            let xml = text("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml")?;
            let prefix = format!("{mc}-");
            let mut v: Vec<String> = xml_versions(&xml)
                .into_iter()
                .filter_map(|s| s.strip_prefix(&prefix).map(String::from))
                .collect();
            v.reverse();
            Ok(v)
        }
        "neoforge" => {
            let prefix = neo_prefix(mc).ok_or_else(|| anyhow!("NeoForge does not support {mc}"))?;
            let j: Value = get_json(
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
            )?;
            let mut v: Vec<String> = j["versions"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .filter(|s| s.starts_with(&prefix))
                .collect();
            v.reverse();
            Ok(v)
        }
        _ => Ok(Vec::new()),
    }
}

fn dir_names(dir: &Path) -> HashSet<String> {
    fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Install a loader on top of an already-installed vanilla `mc` and return the
/// launchable version id.
pub fn install(
    paths: &Paths,
    loader: &str,
    mc: &str,
    loader_version: &str,
    java: &Path,
    rep: Reporter,
) -> Result<String> {
    safe_file_name(loader_version)?;
    rep(Progress::msg("loader", &format!("Installing {loader} {loader_version}")));
    match loader {
        "fabric" | "quilt" => {
            let base = if loader == "fabric" {
                "https://meta.fabricmc.net/v2/versions/loader"
            } else {
                "https://meta.quiltmc.org/v3/versions/loader"
            };
            let url = format!(
                "{base}/{}/{}/profile/json",
                enc(mc),
                enc(loader_version)
            );
            let v: Value = get_json(&url)?;
            let id = v["id"]
                .as_str()
                .ok_or_else(|| anyhow!("loader profile has no id"))?
                .to_string();
            safe_file_name(&id)?;
            write_json_file(&paths.version_json(&id), &v)?;
            Ok(id)
        }
        "forge" | "neoforge" => {
            let (expected, url) = if loader == "forge" {
                (
                    format!("{mc}-forge-{loader_version}"),
                    format!(
                        "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{lv}/forge-{mc}-{lv}-installer.jar",
                        lv = loader_version
                    ),
                )
            } else {
                (
                    format!("neoforge-{loader_version}"),
                    format!(
                        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{lv}/neoforge-{lv}-installer.jar",
                        lv = loader_version
                    ),
                )
            };
            if paths.version_json(&expected).exists() {
                return Ok(expected);
            }
            let installer = paths.tmp().join(format!("{loader}-{loader_version}-installer.jar"));
            download(&url, &installer, None)?;

            // The installer expects a launcher profile file next to the versions folder.
            let profiles = paths.root.join("launcher_profiles.json");
            if !profiles.exists() {
                fs::write(&profiles, br#"{"profiles":{},"version":3}"#)?;
            }
            let before = dir_names(&paths.versions());
            let mut cmd = Command::new(java);
            cmd.arg("-jar")
                .arg(&installer)
                .arg("--installClient")
                .arg(&paths.root)
                .current_dir(&paths.root);
            hide_window(&mut cmd);
            let out = cmd.output()?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                let tail: String = err
                    .lines()
                    .rev()
                    .take(6)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                bail!("{loader} installer failed:\n{tail}");
            }
            let _ = fs::remove_file(&installer);
            if paths.version_json(&expected).exists() {
                return Ok(expected);
            }
            // Older Forge installers name the version differently; pick whatever is new.
            let after = dir_names(&paths.versions());
            after
                .difference(&before)
                .find(|n| paths.version_json(n).exists())
                .cloned()
                .ok_or_else(|| anyhow!("{loader} installer finished but no version was created"))
        }
        _ => bail!("unknown loader {loader}"),
    }
}
