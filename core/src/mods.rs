//! Content browsing and installing from Modrinth and CurseForge:
//! search, project pages (description + screenshots), version lists, and installs of
//! either the newest compatible file or one specific version the user picked.

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

// ---- Modrinth: search ---------------------------------------------------------------

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
    if (kind != "mod" && kind != "modpack") || loader.is_empty() || loader == "vanilla" {
        return String::new();
    }
    let list = if loader == "quilt" && kind == "mod" {
        json!(["quilt", "fabric"])
    } else {
        json!([loader])
    };
    format!("&loaders={}", enc(&list.to_string()))
}

// ---- Modrinth: project page ----------------------------------------------------------

fn mr_details(id: &str) -> Result<Value> {
    let p: Value = get_json(&format!("{MR}/project/{}", enc(id)))?;
    let members: Vec<Value> =
        get_json::<Vec<Value>>(&format!("{MR}/project/{}/members", enc(id))).unwrap_or_default();
    let team: Vec<Value> = members
        .iter()
        .map(|m| {
            json!({
                "name": m["user"]["username"],
                "role": m["role"],
                "avatar": m["user"]["avatar_url"],
            })
        })
        .collect();

    let mut links: Vec<Value> = Vec::new();
    for (label, key) in [
        ("Source", "source_url"),
        ("Issues", "issues_url"),
        ("Wiki", "wiki_url"),
        ("Discord", "discord_url"),
    ] {
        if let Some(u) = p[key].as_str() {
            links.push(json!({ "label": label, "url": u }));
        }
    }
    for d in p["donation_urls"].as_array().into_iter().flatten() {
        links.push(json!({
            "label": d["platform"].as_str().unwrap_or("Support"),
            "url": d["url"]
        }));
    }

    let gallery: Vec<Value> = p["gallery"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|g| {
            json!({
                "url": g["url"],
                "full": g["raw_url"],
                "title": g["title"],
                "description": g["description"],
            })
        })
        .collect();

    let slug = p["slug"].as_str().unwrap_or(id);
    let kind = p["project_type"].as_str().unwrap_or("mod");
    Ok(json!({
        "source": "modrinth",
        "id": p["id"],
        "title": p["title"],
        "summary": p["description"],
        "body": p["body"],
        "body_format": "markdown",
        "icon": p["icon_url"],
        "gallery": gallery,
        "categories": p["categories"],
        "loaders": p["loaders"],
        "game_versions": p["game_versions"],
        "downloads": p["downloads"],
        "followers": p["followers"],
        "license": p["license"]["name"],
        "published": p["published"],
        "updated": p["updated"],
        "links": links,
        "team": team,
        "kind": kind,
        "web_url": format!("https://modrinth.com/{kind}/{slug}"),
    }))
}

fn mr_versions(project: &str, kind: &str, mc: &str, loader: &str) -> Result<Value> {
    let mut url = format!("{MR}/project/{}/version?", enc(project));
    if !mc.is_empty() {
        url.push_str(&format!("game_versions={}", enc(&json!([mc]).to_string())));
    }
    url.push_str(&mr_loader_filter(kind, loader));
    let list: Vec<Value> = get_json(&url)?;
    let out: Vec<Value> = list
        .iter()
        .take(150)
        .map(|v| {
            let files: Vec<Value> = v["files"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|f| json!({ "name": f["filename"], "size": f["size"] }))
                .collect();
            json!({
                "id": v["id"],
                "name": v["name"],
                "number": v["version_number"],
                "kind": v["version_type"],
                "game_versions": v["game_versions"],
                "loaders": v["loaders"],
                "downloads": v["downloads"],
                "published": v["date_published"],
                "changelog": v["changelog"],
                "files": files,
            })
        })
        .collect();
    Ok(Value::Array(out))
}

// ---- Modrinth: install ----------------------------------------------------------------

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
    let files = version["files"]
        .as_array()
        .ok_or_else(|| anyhow!("version has no files"))?;
    files
        .iter()
        .find(|f| f["primary"].as_bool().unwrap_or(false))
        .or_else(|| files.first())
        .ok_or_else(|| anyhow!("version has no files"))
}

/// Write one Modrinth version's file into the game folder, then its required dependencies.
fn mr_install_ver(
    ver: &Value,
    kind: &str,
    mc: &str,
    loader: &str,
    game_dir: &Path,
    depth: u32,
    installed: &mut Vec<String>,
) -> Result<()> {
    let file = mr_primary_file(ver)?;
    let name = safe_file_name(file["filename"].as_str().unwrap_or(""))?;
    let url = file["url"]
        .as_str()
        .ok_or_else(|| anyhow!("file has no download url"))?;
    let dest = game_dir.join(folder(kind)?).join(&name);
    download(url, &dest, file["hashes"]["sha1"].as_str())?;
    installed.push(name);

    if depth < 3 && kind == "mod" {
        for dep in ver["dependencies"].as_array().into_iter().flatten() {
            if dep["dependency_type"] != "required" {
                continue;
            }
            // A dependency that cannot be resolved must not fail the main install.
            if let Some(vid) = dep["version_id"].as_str() {
                if let Ok(dv) = get_json::<Value>(&format!("{MR}/version/{}", enc(vid))) {
                    let _ = mr_install_ver(&dv, kind, mc, loader, game_dir, depth + 1, installed);
                    continue;
                }
            }
            if let Some(pid) = dep["project_id"].as_str() {
                if let Ok(dv) = mr_latest_version(pid, kind, mc, loader) {
                    let _ = mr_install_ver(&dv, kind, mc, loader, game_dir, depth + 1, installed);
                }
            }
        }
    }
    Ok(())
}

// ---- CurseForge -----------------------------------------------------------------------

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

fn cf_loader_name(id: u64) -> Option<&'static str> {
    match id {
        1 => Some("forge"),
        4 => Some("fabric"),
        5 => Some("quilt"),
        6 => Some("neoforge"),
        _ => None,
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

fn cf_details(key: &str, id: &str) -> Result<Value> {
    let m = cf_get(key, &format!("{CF}/mods/{}", enc(id)))?;
    let d = &m["data"];
    let body = cf_get(key, &format!("{CF}/mods/{}/description", enc(id)))
        .ok()
        .and_then(|v| v["data"].as_str().map(String::from))
        .unwrap_or_default();

    let mut game_versions: Vec<String> = Vec::new();
    let mut loaders: Vec<String> = Vec::new();
    for ix in d["latestFilesIndexes"].as_array().into_iter().flatten() {
        if let Some(gv) = ix["gameVersion"].as_str() {
            if !game_versions.iter().any(|x| x == gv) {
                game_versions.push(gv.to_string());
            }
        }
        if let Some(l) = ix["modLoader"].as_u64().and_then(cf_loader_name) {
            if !loaders.iter().any(|x| x == l) {
                loaders.push(l.to_string());
            }
        }
    }
    game_versions.reverse();

    let mut links: Vec<Value> = Vec::new();
    for (label, key) in [
        ("Source", "sourceUrl"),
        ("Issues", "issuesUrl"),
        ("Wiki", "wikiUrl"),
    ] {
        if let Some(u) = d["links"][key].as_str() {
            if !u.is_empty() {
                links.push(json!({ "label": label, "url": u }));
            }
        }
    }
    let team: Vec<Value> = d["authors"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|a| json!({ "name": a["name"], "role": "Author", "avatar": Value::Null }))
        .collect();
    let gallery: Vec<Value> = d["screenshots"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            json!({
                "url": s["thumbnailUrl"],
                "full": s["url"],
                "title": s["title"],
                "description": s["description"],
            })
        })
        .collect();
    let categories: Vec<String> = d["categories"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["name"].as_str().map(|s| s.to_lowercase()))
        .collect();

    Ok(json!({
        "source": "curseforge",
        "id": id,
        "title": d["name"],
        "summary": d["summary"],
        "body": body,
        "body_format": "html",
        "icon": d["logo"]["thumbnailUrl"],
        "gallery": gallery,
        "categories": categories,
        "loaders": loaders,
        "game_versions": game_versions,
        "downloads": d["downloadCount"],
        "followers": d["thumbsUpCount"],
        "license": Value::Null,
        "published": d["dateCreated"],
        "updated": d["dateModified"],
        "links": links,
        "team": team,
        "kind": "mod",
        "web_url": d["links"]["websiteUrl"],
    }))
}

fn cf_versions(key: &str, project: &str, kind: &str, mc: &str, loader: &str) -> Result<Value> {
    let mut url = format!("{CF}/mods/{}/files?pageSize=50", enc(project));
    if !mc.is_empty() {
        url.push_str(&format!("&gameVersion={}", enc(mc)));
    }
    let lid = cf_loader(loader);
    if lid != 0 && kind == "mod" {
        url.push_str(&format!("&modLoaderType={lid}"));
    }
    let r = cf_get(key, &url)?;
    let out: Vec<Value> = r["data"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            let mut gv: Vec<String> = Vec::new();
            let mut ld: Vec<String> = Vec::new();
            for g in f["gameVersions"].as_array().into_iter().flatten() {
                if let Some(s) = g.as_str() {
                    if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        gv.push(s.to_string());
                    } else {
                        ld.push(s.to_lowercase());
                    }
                }
            }
            let kind = match f["releaseType"].as_u64() {
                Some(2) => "beta",
                Some(3) => "alpha",
                _ => "release",
            };
            json!({
                "id": f["id"].as_u64().map(|n| n.to_string()).unwrap_or_default(),
                "name": f["displayName"],
                "number": f["fileName"],
                "kind": kind,
                "game_versions": gv,
                "loaders": ld,
                "downloads": f["downloadCount"],
                "published": f["fileDate"],
                "changelog": "",
                "files": [json!({ "name": f["fileName"], "size": f["fileLength"] })],
            })
        })
        .collect();
    Ok(Value::Array(out))
}

/// Write one CurseForge file (a `files` API entry) and its required dependencies.
#[allow(clippy::too_many_arguments)]
fn cf_install_file(
    key: &str,
    file: &Value,
    kind: &str,
    mc: &str,
    loader: &str,
    game_dir: &Path,
    depth: u32,
    installed: &mut Vec<String>,
) -> Result<()> {
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
                    let _ = cf_install_latest(
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

#[allow(clippy::too_many_arguments)]
fn cf_install_latest(
    key: &str,
    project: &str,
    kind: &str,
    mc: &str,
    loader: &str,
    game_dir: &Path,
    depth: u32,
    installed: &mut Vec<String>,
) -> Result<()> {
    let mut url = format!(
        "{CF}/mods/{}/files?pageSize=1&gameVersion={}",
        enc(project),
        enc(mc)
    );
    let lid = cf_loader(loader);
    if lid != 0 && kind == "mod" {
        url.push_str(&format!("&modLoaderType={lid}"));
    }
    let r = cf_get(key, &url)?;
    let file = r["data"]
        .get(0)
        .ok_or_else(|| anyhow!("No version of this project works with {mc} {loader}"))?;
    cf_install_file(key, file, kind, mc, loader, game_dir, depth, installed)
}

// ---- unified entry points -------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
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

/// Everything the project page shows: description, screenshots, links, creators.
pub fn project_details(source: &str, cf_key: &str, id: &str) -> Result<Value> {
    match source {
        "curseforge" => cf_details(cf_key, id),
        _ => mr_details(id),
    }
}

/// Versions of a project, newest first, optionally narrowed to a game version / loader.
pub fn versions(
    source: &str,
    cf_key: &str,
    project: &str,
    kind: &str,
    mc: &str,
    loader: &str,
) -> Result<Value> {
    match source {
        "curseforge" => cf_versions(cf_key, project, kind, mc, loader),
        _ => mr_versions(project, kind, mc, loader),
    }
}

/// Install the newest compatible version (and its required dependencies).
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
        "curseforge" => cf_install_latest(
            cf_key,
            project,
            kind,
            mc,
            loader,
            game_dir,
            0,
            &mut installed,
        )?,
        _ => {
            let ver = mr_latest_version(project, kind, mc, loader)?;
            mr_install_ver(&ver, kind, mc, loader, game_dir, 0, &mut installed)?;
        }
    }
    Ok(installed)
}

/// Install one specific version the user picked, whatever its game version or age.
#[allow(clippy::too_many_arguments)]
pub fn install_version(
    source: &str,
    cf_key: &str,
    project: &str,
    version_id: &str,
    kind: &str,
    mc: &str,
    loader: &str,
    game_dir: &Path,
) -> Result<Vec<String>> {
    let mut installed = Vec::new();
    match source {
        "curseforge" => {
            let r = cf_get(
                cf_key,
                &format!("{CF}/mods/{}/files/{}", enc(project), enc(version_id)),
            )?;
            cf_install_file(
                cf_key,
                &r["data"],
                kind,
                mc,
                loader,
                game_dir,
                0,
                &mut installed,
            )?;
        }
        _ => {
            let ver: Value = get_json(&format!("{MR}/version/{}", enc(version_id)))?;
            mr_install_ver(&ver, kind, mc, loader, game_dir, 0, &mut installed)?;
        }
    }
    Ok(installed)
}
