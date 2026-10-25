//! Content browsing and installing from Modrinth and CurseForge.

use crate::util::*;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::path::Path;

const MR: &str = "https://api.modrinth.com/v2";
const CF: &str = "https://api.curseforge.com/v1";

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

/// Folder inside the game directory for a project type.
pub fn folder(kind: &str) -> Result<&'static str> {
    match kind {
        "mod" => Ok("mods"),
        "resourcepack" => Ok("resourcepacks"),
        "shader" => Ok("shaderpacks"),
        _ => bail!("unknown content type {kind}"),
    }
}

// ---- Modrinth -----------------------------------------------------------------------

pub fn mr_search(
    query: &str,
    kind: &str,
    mc: &str,
    loader: &str,
    sort: &str,
    offset: u32,
) -> Result<Value> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{kind}")]];
    if !mc.is_empty() {
        facets.push(vec![format!("versions:{mc}")]);
    }
    if !loader.is_empty() && loader != "vanilla" && (kind == "mod" || kind == "modpack") {
        facets.push(vec![format!("categories:{loader}")]);
    }
    let index = match sort {
        "downloads" | "follows" | "newest" | "updated" => sort,
        _ => "relevance",
    };
    let url = format!(
        "{MR}/search?query={}&facets={}&limit=20&offset={}&index={}",
        enc(query),
        enc(&serde_json::to_string(&facets)?),
        offset,
        index
    );
    let r: Value = get_json(&url)?;
    let hits: Vec<Value> = r["hits"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|h| {
            json!({
                "source": "modrinth",
                "id": h["project_id"],
                "title": h["title"],
                "description": h["description"],
                "icon": h["icon_url"],
                "downloads": h["downloads"],
                "author": h["author"],
                "kind": h["project_type"],
            })
        })
        .collect();
    Ok(json!({ "hits": hits, "total": r["total_hits"] }))
}

fn mr_loader_filter(kind: &str, loader: &str) -> String {
    if kind != "mod" || loader.is_empty() || loader == "vanilla" {
        return String::new();
    }
    let list = if loader == "quilt" {
        json!(["quilt", "fabric"])
    } else {
        json!([loader])
    };
    format!("&loaders={}", enc(&list.to_string()))
}

pub fn mr_latest_version(project: &str, kind: &str, mc: &str, loader: &str) -> Result<Value> {
    let mut url = format!("{MR}/project/{}/version?", enc(project));
    url.push_str(&format!("game_versions={}", enc(&json!([mc]).to_string())));
    url.push_str(&mr_loader_filter(kind, loader));
    let versions: Vec<Value> = get_json(&url)?;
    versions
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("No version of this project works with {mc} {loader}"))
}

fn mr_primary_file(version: &Value) -> Result<&Value> {
    let files = version["files"].as_array().ok_or_else(|| anyhow!("version has no files"))?;
    files
        .iter()
        .find(|f| f["primary"].as_bool().unwrap_or(false))
        .or_else(|| files.first())
        .ok_or_else(|| anyhow!("version has no files"))
}

fn mr_install(
    project: &str,
    kind: &str,
    mc: &str,
    loader: &str,
    game_dir: &Path,
    depth: u32,
    installed: &mut Vec<String>,
) -> Result<()> {
    let ver = mr_latest_version(project, kind, mc, loader)?;
    let file = mr_primary_file(&ver)?;
    let name = safe_file_name(file["filename"].as_str().unwrap_or(""))?;
    let url = file["url"].as_str().ok_or_else(|| anyhow!("file has no download url"))?;
    let dest = game_dir.join(folder(kind)?).join(&name);
    download(url, &dest, file["hashes"]["sha1"].as_str())?;
    installed.push(name);

    if depth < 3 && kind == "mod" {
        for dep in ver["dependencies"].as_array().into_iter().flatten() {
            if dep["dependency_type"] == "required" {
                if let Some(pid) = dep["project_id"].as_str() {
                    // A missing optional-in-practice dependency must not fail the main install.
                    let _ = mr_install(pid, kind, mc, loader, game_dir, depth + 1, installed);
                }
            }
        }
    }
    Ok(())
}

// ---- CurseForge ---------------------------------------------------------------------

fn cf_get(key: &str, url: &str) -> Result<Value> {
    if key.trim().is_empty() {
        bail!("CurseForge needs an API key. Add one in Settings (free at console.curseforge.com).");
    }
    let r = http()
        .get(url)
        .set("x-api-key", key.trim())
        .set("Accept", "application/json")
        .call()
        .map_err(http_err)?;
    Ok(r.into_json::<Value>()?)
}

fn cf_class(kind: &str) -> Result<u32> {
    match kind {
        "mod" => Ok(6),
        "resourcepack" => Ok(12),
        "shader" => Ok(6552),
        "modpack" => Ok(4471),
        _ => bail!("unknown content type {kind}"),
    }
}

fn cf_loader(loader: &str) -> u32 {
    match loader {
        "forge" => 1,
        "fabric" => 4,
        "quilt" => 5,
        "neoforge" => 6,
        _ => 0,
    }
}

pub fn cf_search(
    key: &str,
    query: &str,
    kind: &str,
    mc: &str,
    loader: &str,
    sort: &str,
    offset: u32,
) -> Result<Value> {
    let sort_field = match sort {
        "downloads" => 6,
        "updated" => 3,
        "newest" => 11,
        _ => 2,
    };
    let mut url = format!(
        "{CF}/mods/search?gameId=432&classId={}&searchFilter={}&sortField={}&sortOrder=desc&pageSize=20&index={}",
        cf_class(kind)?,
        enc(query),
        sort_field,
        offset
    );
    if !mc.is_empty() {
        url.push_str(&format!("&gameVersion={}", enc(mc)));
    }
    let lid = cf_loader(loader);
    if lid != 0 && kind == "mod" {
        url.push_str(&format!("&modLoaderType={lid}"));
    }
    let r = cf_get(key, &url)?;
    let hits: Vec<Value> = r["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|h| {
            json!({
                "source": "curseforge",
                "id": h["id"].as_u64().map(|n| n.to_string()).unwrap_or_default(),
                "title": h["name"],
                "description": h["summary"],
                "icon": h["logo"]["thumbnailUrl"],
                "downloads": h["downloadCount"],
                "author": h["authors"][0]["name"],
                "kind": kind,
            })
        })
        .collect();
    Ok(json!({ "hits": hits, "total": r["pagination"]["totalCount"] }))
}

fn cf_install(
    key: &str,
    project: &str,
    kind: &str,
    mc: &str,
    loader: &str,
    game_dir: &Path,
    depth: u32,
    installed: &mut Vec<String>,
) -> Result<()> {
    let mut url = format!("{CF}/mods/{}/files?pageSize=1&gameVersion={}", enc(project), enc(mc));
    let lid = cf_loader(loader);
    if lid != 0 && kind == "mod" {
        url.push_str(&format!("&modLoaderType={lid}"));
    }
    let r = cf_get(key, &url)?;
    let file = r["data"]
        .get(0)
        .ok_or_else(|| anyhow!("No version of this project works with {mc} {loader}"))?;
    let name = safe_file_name(file["fileName"].as_str().unwrap_or(""))?;
    let id = file["id"].as_u64().unwrap_or(0);
    // Some authors disable third-party downloads; the CDN path still works for launchers.
    let dl = match file["downloadUrl"].as_str() {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => format!(
            "https://edge.forgecdn.net/files/{}/{}/{}",
            id / 1000,
            id % 1000,
            enc(&name)
        ),
    };
    let sha1 = file["hashes"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|h| h["algo"] == 1)
        .and_then(|h| h["value"].as_str());
    download(&dl, &game_dir.join(folder(kind)?).join(&name), sha1)?;
    installed.push(name);

    if depth < 3 && kind == "mod" {
        for dep in file["dependencies"].as_array().into_iter().flatten() {
            // relationType 3 = required dependency
            if dep["relationType"] == 3 {
                if let Some(mid) = dep["modId"].as_u64() {
                    let _ = cf_install(
                        key,
                        &mid.to_string(),
                        kind,
                        mc,
                        loader,
                        game_dir,
                        depth + 1,
                        installed,
                    );
                }
            }
        }
    }
    Ok(())
}

// ---- unified entry points -------------------------------------------------------------

pub fn search(
    source: &str,
    cf_key: &str,
    query: &str,
    kind: &str,
    mc: &str,
    loader: &str,
    sort: &str,
    offset: u32,
) -> Result<Value> {
    match source {
        "curseforge" => cf_search(cf_key, query, kind, mc, loader, sort, offset),
        _ => mr_search(query, kind, mc, loader, sort, offset),
    }
}

/// Install a project (and its required dependencies) into a game directory.
/// Returns the file names that were written.
pub fn install(
    source: &str,
    cf_key: &str,
    project: &str,
    kind: &str,
    mc: &str,
    loader: &str,
    game_dir: &Path,
) -> Result<Vec<String>> {
    let mut installed = Vec::new();
    match source {
        "curseforge" => cf_install(cf_key, project, kind, mc, loader, game_dir, 0, &mut installed)?,
        _ => mr_install(project, kind, mc, loader, game_dir, 0, &mut installed)?,
    }
    Ok(installed)
}
