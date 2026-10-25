//! Vanilla version handling: manifests, inheritance merge, libraries, assets, natives.
//! Version JSON is handled as `serde_json::Value` on purpose: loaders (Fabric, Forge...)
//! add all sorts of extra keys and we only need a handful of them.

use crate::config::Paths;
use crate::util::*;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";

pub fn list_versions() -> Result<Vec<Value>> {
    let m: Value = get_json(MANIFEST)?;
    Ok(m["versions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|v| json!({ "id": v["id"], "type": v["type"], "released": v["releaseTime"] }))
        .collect())
}

/// Download the vanilla version JSON for `id` if it is not on disk yet.
pub fn ensure_vanilla_json(paths: &Paths, id: &str) -> Result<()> {
    safe_file_name(id)?;
    let file = paths.version_json(id);
    if file.exists() {
        return Ok(());
    }
    let m: Value = get_json(MANIFEST)?;
    let url = m["versions"]
        .as_array()
        .and_then(|a| a.iter().find(|v| v["id"] == id))
        .and_then(|v| v["url"].as_str())
        .map(String::from)
        .ok_or_else(|| anyhow!("unknown Minecraft version {id}"))?;
    download(&url, &file, None)
}

/// Pre-1.13 versions use a single `minecraftArguments` string; convert it so the rest
/// of the code only has to know about the modern `arguments` shape.
fn normalize_args(v: &mut Value) {
    if let Some(s) = v["minecraftArguments"].as_str().map(String::from) {
        let list: Vec<Value> = s
            .split_whitespace()
            .map(|x| Value::String(x.to_string()))
            .collect();
        v["arguments"] = json!({ "game": list, "jvm": [] });
        if let Some(o) = v.as_object_mut() {
            o.remove("minecraftArguments");
        }
    }
}

pub fn read_version(paths: &Paths, id: &str) -> Result<Value> {
    safe_file_name(id)?;
    let bytes = fs::read(paths.version_json(id))
        .with_context(|| format!("version {id} is not installed"))?;
    let mut v: Value = serde_json::from_slice(&bytes)?;
    normalize_args(&mut v);
    Ok(v)
}

/// Follow `inheritsFrom` to the vanilla version that owns the client jar.
pub fn base_id(paths: &Paths, id: &str) -> Result<String> {
    let mut cur = id.to_string();
    for _ in 0..6 {
        let v = read_version(paths, &cur)?;
        match v["inheritsFrom"].as_str() {
            Some(p) => cur = p.to_string(),
            None => return Ok(cur),
        }
    }
    bail!("version inheritance is too deep")
}

fn lib_key(l: &Value) -> String {
    let name = l["name"].as_str().unwrap_or("");
    let p: Vec<&str> = name.split(':').collect();
    if p.len() >= 4 {
        format!("{}:{}:{}", p[0], p[1], p[3])
    } else if p.len() >= 2 {
        format!("{}:{}", p[0], p[1])
    } else {
        name.to_string()
    }
}

fn merge(mut parent: Value, child: Value) -> Value {
    let mut libs = child["libraries"].as_array().cloned().unwrap_or_default();
    let seen: HashSet<String> = libs.iter().map(lib_key).collect();
    for l in parent["libraries"].as_array().cloned().unwrap_or_default() {
        if !seen.contains(&lib_key(&l)) {
            libs.push(l);
        }
    }
    let mut args = json!({ "game": [], "jvm": [] });
    for part in ["game", "jvm"] {
        let mut a = parent["arguments"][part].as_array().cloned().unwrap_or_default();
        a.extend(child["arguments"][part].as_array().cloned().unwrap_or_default());
        args[part] = Value::Array(a);
    }
    if let Some(obj) = child.as_object() {
        for (k, val) in obj {
            if k != "libraries" && k != "arguments" {
                parent[k.as_str()] = val.clone();
            }
        }
    }
    parent["libraries"] = Value::Array(libs);
    parent["arguments"] = args;
    parent
}

/// The fully merged version JSON (loader on top of vanilla).
pub fn resolve(paths: &Paths, id: &str) -> Result<Value> {
    resolve_depth(paths, id, 0)
}

fn resolve_depth(paths: &Paths, id: &str, depth: u32) -> Result<Value> {
    if depth > 6 {
        bail!("version inheritance is too deep");
    }
    let v = read_version(paths, id)?;
    match v["inheritsFrom"].as_str().map(String::from) {
        Some(parent) => {
            let p = resolve_depth(paths, &parent, depth + 1)?;
            Ok(merge(p, v))
        }
        None => Ok(v),
    }
}

pub fn rules_ok(rules: Option<&Value>, features: &[&str]) -> bool {
    let arr = match rules.and_then(|r| r.as_array()) {
        Some(a) => a,
        None => return true,
    };
    let mut allowed = false;
    for r in arr {
        let mut applies = true;
        if let Some(os) = r.get("os") {
            if let Some(n) = os["name"].as_str() {
                if n != os_name() {
                    applies = false;
                }
            }
            if let Some(a) = os["arch"].as_str() {
                if a != current_arch() {
                    applies = false;
                }
            }
        }
        if let Some(f) = r.get("features").and_then(|f| f.as_object()) {
            for (k, val) in f {
                let want = val.as_bool().unwrap_or(false);
                let have = features.contains(&k.as_str());
                if want != have {
                    applies = false;
                }
            }
        }
        if applies {
            allowed = r["action"].as_str() == Some("allow");
        }
    }
    allowed
}

#[derive(Clone, Debug)]
pub struct LibFile {
    pub url: String,
    pub rel: String,
    pub sha1: Option<String>,
    pub native: bool,
}

fn maven_path(name: &str) -> Option<String> {
    let (n, ext) = match name.split_once('@') {
        Some((a, e)) => (a, e),
        None => (name, "jar"),
    };
    let p: Vec<&str> = n.split(':').collect();
    if p.len() < 3 {
        return None;
    }
    let group = p[0].replace('.', "/");
    let (art, ver) = (p[1], p[2]);
    let cl = if p.len() > 3 {
        format!("-{}", p[3])
    } else {
        String::new()
    };
    Some(format!("{group}/{art}/{ver}/{art}-{ver}{cl}.{ext}"))
}

fn file_from(a: &Value, name: &str) -> Option<LibFile> {
    let rel = match a["path"].as_str() {
        Some(p) => p.to_string(),
        None => maven_path(name)?,
    };
    Some(LibFile {
        url: a["url"].as_str().unwrap_or("").to_string(),
        rel,
        sha1: a["sha1"].as_str().map(String::from),
        native: false,
    })
}

/// Every file the libraries section asks for on this OS.
pub fn lib_files(v: &Value) -> Vec<LibFile> {
    let mut out = Vec::new();
    let bits = if cfg!(target_pointer_width = "64") { "64" } else { "32" };
    for lib in v["libraries"].as_array().into_iter().flatten() {
        if !rules_ok(lib.get("rules"), &[]) {
            continue;
        }
        let name = lib["name"].as_str().unwrap_or("");

        // Old-style natives: a classifier jar that has to be unpacked.
        if let Some(cl) = lib["natives"][os_name()].as_str() {
            let cl = cl.replace("${arch}", bits);
            if let Some(a) = lib["downloads"]["classifiers"].get(cl.as_str()) {
                if let Some(mut f) = file_from(a, name) {
                    f.native = true;
                    out.push(f);
                }
            }
        }

        if let Some(a) = lib["downloads"].get("artifact") {
            if let Some(f) = file_from(a, name) {
                out.push(f);
            }
        } else if lib.get("downloads").is_none() {
            // Maven-style entry (Fabric, Quilt, some Forge libs).
            if let Some(rel) = maven_path(name) {
                let mut base = lib["url"].as_str().unwrap_or(LIBRARIES_URL).to_string();
                if !base.ends_with('/') {
                    base.push('/');
                }
                out.push(LibFile {
                    url: format!("{base}{rel}"),
                    rel,
                    sha1: None,
                    native: false,
                });
            }
        }
    }
    out
}

pub fn natives_dir(paths: &Paths, id: &str) -> PathBuf {
    paths.version_dir(id).join("natives")
}

pub fn client_jar(paths: &Paths, id: &str) -> Result<PathBuf> {
    let base = base_id(paths, id)?;
    Ok(paths.version_dir(&base).join(format!("{base}.jar")))
}

fn extract_natives(jar: &Path, dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    let mut z = zip::ZipArchive::new(fs::File::open(jar)?)?;
    for i in 0..z.len() {
        let mut f = z.by_index(i)?;
        if f.is_dir() {
            continue;
        }
        let name = match f.enclosed_name() {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with("META-INF") {
            continue;
        }
        let fname = match name.file_name() {
            Some(n) => n.to_owned(),
            None => continue,
        };
        let out = dir.join(fname);
        if out.exists() {
            continue;
        }
        let mut o = fs::File::create(out)?;
        std::io::copy(&mut f, &mut o)?;
    }
    Ok(())
}

/// Download everything `id` needs to run: client jar, libraries, assets, natives.
pub fn install_version(paths: &Paths, id: &str, rep: Reporter) -> Result<()> {
    let v = resolve(paths, id)?;
    rep(Progress::msg("game", "Checking game files"));

    // Client jar (owned by the vanilla base version).
    let jar = client_jar(paths, id)?;
    let client = &v["downloads"]["client"];
    match client["url"].as_str() {
        Some(url) => download(url, &jar, client["sha1"].as_str())?,
        None => {
            if !jar.exists() {
                bail!("no client download available for {id}");
            }
        }
    }

    // Libraries.
    let libs = lib_files(&v);
    let todo: Vec<&LibFile> = libs.iter().filter(|l| !l.url.is_empty()).collect();
    let total = todo.len() as u64;
    let done = AtomicU64::new(0);
    par_each(&todo, 8, |l| {
        download(
            &l.url,
            &paths.libraries().join(safe_rel(&l.rel)?),
            l.sha1.as_deref(),
        )?;
        let d = done.fetch_add(1, Ordering::Relaxed) + 1;
        rep(Progress::new("libraries", d, total, "Downloading libraries"));
        Ok(())
    })?;

    let ndir = natives_dir(paths, id);
    for l in libs.iter().filter(|l| l.native) {
        extract_natives(&paths.libraries().join(safe_rel(&l.rel)?), &ndir)?;
    }

    // Assets.
    let ai = &v["assetIndex"];
    let aid = ai["id"]
        .as_str()
        .ok_or_else(|| anyhow!("version has no asset index"))?;
    let idx_path = paths.assets().join("indexes").join(format!("{aid}.json"));
    download(ai["url"].as_str().unwrap_or(""), &idx_path, ai["sha1"].as_str())?;
    let idx: Value = serde_json::from_slice(&fs::read(&idx_path)?)?;
    let hashes: Vec<String> = idx["objects"]
        .as_object()
        .map(|o| {
            o.values()
                .filter_map(|x| x["hash"].as_str().map(String::from))
                .filter(|h| h.len() > 2)
                .collect()
        })
        .unwrap_or_default();
    let total = hashes.len() as u64;
    let done = AtomicU64::new(0);
    par_each(&hashes, 16, |h| {
        let target = paths.assets().join("objects").join(&h[..2]).join(h);
        if !target.exists() {
            let url = format!("https://resources.download.minecraft.net/{}/{}", &h[..2], h);
            download(&url, &target, Some(h))?;
        }
        let d = done.fetch_add(1, Ordering::Relaxed) + 1;
        if d % 20 == 0 || d == total {
            rep(Progress::new("assets", d, total, "Downloading assets"));
        }
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.16.9").unwrap(),
            "net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.3:natives-linux").unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar"
        );
        assert!(maven_path("broken").is_none());
    }

    #[test]
    fn rules_allow_and_deny() {
        let deny_this_os = json!([{ "action": "allow" }, { "action": "disallow", "os": { "name": os_name() } }]);
        assert!(!rules_ok(Some(&deny_this_os), &[]));
        let allow_other_only = json!([{ "action": "allow", "os": { "name": "no-such-os" } }]);
        assert!(!rules_ok(Some(&allow_other_only), &[]));
        assert!(rules_ok(None, &[]));
    }

    #[test]
    fn loader_libs_override_vanilla_and_args_concat() {
        let parent = json!({
            "id": "1.21.1", "mainClass": "net.minecraft.client.main.Main",
            "libraries": [{ "name": "a:b:1" }, { "name": "c:d:1" }],
            "arguments": { "game": ["--x"], "jvm": ["-Da"] }
        });
        let child = json!({
            "id": "fabric", "mainClass": "net.fabricmc.Knot",
            "libraries": [{ "name": "a:b:2" }],
            "arguments": { "game": [], "jvm": ["-Db"] }
        });
        let m = merge(parent, child);
        assert_eq!(m["mainClass"], "net.fabricmc.Knot");
        let libs: Vec<&str> = m["libraries"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
        assert_eq!(libs, vec!["a:b:2", "c:d:1"]);
        assert_eq!(m["arguments"]["jvm"].as_array().unwrap().len(), 2);
    }
}
