//! Turns an installed version + an account into a Java command line, and starts it.

use crate::config::Paths;
use crate::mojang;
use crate::util::*;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub struct LaunchCtx<'a> {
    pub version_id: &'a str,
    pub player: &'a str,
    pub uuid: &'a str,
    pub token: &'a str,
    pub user_type: &'a str,
    pub game_dir: &'a Path,
    pub memory_mb: u32,
    pub extra_jvm: &'a str,
}

fn subst(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = s.to_string();
    for (k, val) in vars {
        out = out.replace(&format!("${{{k}}}"), val);
    }
    out
}

fn collect_args(arr: &[Value], vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for a in arr {
        match a {
            Value::String(s) => out.push(subst(s, vars)),
            Value::Object(_) => {
                if !mojang::rules_ok(a.get("rules"), &[]) {
                    continue;
                }
                match &a["value"] {
                    Value::String(s) => out.push(subst(s, vars)),
                    Value::Array(list) => {
                        for x in list {
                            if let Some(s) = x.as_str() {
                                out.push(subst(s, vars));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

/// Build the arguments to pass to `java`.
pub fn build(paths: &Paths, ctx: &LaunchCtx) -> Result<Vec<String>> {
    let v = mojang::resolve(paths, ctx.version_id)?;
    let jar = mojang::client_jar(paths, ctx.version_id)?;

    let mut seen = HashSet::new();
    let mut cp: Vec<String> = Vec::new();
    for l in mojang::lib_files(&v).iter().filter(|l| !l.native) {
        if seen.insert(l.rel.clone()) {
            cp.push(paths.libraries().join(&l.rel).to_string_lossy().into_owned());
        }
    }
    cp.push(jar.to_string_lossy().into_owned());
    let classpath = cp.join(cp_sep());

    let natives = mojang::natives_dir(paths, ctx.version_id);
    let assets_id = v["assetIndex"]["id"].as_str().unwrap_or("legacy").to_string();
    let assets_root = paths.assets().to_string_lossy().into_owned();

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", ctx.player.to_string());
    vars.insert("version_name", ctx.version_id.to_string());
    vars.insert("game_directory", ctx.game_dir.to_string_lossy().into_owned());
    vars.insert("assets_root", assets_root.clone());
    vars.insert("game_assets", format!("{assets_root}/virtual/legacy"));
    vars.insert("assets_index_name", assets_id);
    vars.insert("auth_uuid", ctx.uuid.to_string());
    vars.insert("auth_access_token", ctx.token.to_string());
    vars.insert("auth_session", format!("token:{}:{}", ctx.token, ctx.uuid));
    vars.insert("clientid", String::new());
    vars.insert("auth_xuid", String::new());
    vars.insert("user_type", ctx.user_type.to_string());
    vars.insert("user_properties", "{}".to_string());
    vars.insert(
        "version_type",
        v["type"].as_str().unwrap_or("release").to_string(),
    );
    vars.insert("natives_directory", natives.to_string_lossy().into_owned());
    vars.insert("launcher_name", "SpiderflayerLauncher".to_string());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").to_string());
    vars.insert("classpath", classpath.clone());
    vars.insert("classpath_separator", cp_sep().to_string());
    vars.insert(
        "library_directory",
        paths.libraries().to_string_lossy().into_owned(),
    );

    let jvm_list = v["arguments"]["jvm"].as_array().cloned().unwrap_or_default();
    let game_list = v["arguments"]["game"].as_array().cloned().unwrap_or_default();
    let mut jvm = collect_args(&jvm_list, &vars);
    if !jvm.iter().any(|a| a == "-cp" || a == "-classpath") {
        jvm.push(format!("-Djava.library.path={}", natives.to_string_lossy()));
        jvm.push("-cp".to_string());
        jvm.push(classpath);
    }
    let game = collect_args(&game_list, &vars);
    let main_class = v["mainClass"]
        .as_str()
        .ok_or_else(|| anyhow!("version has no main class"))?
        .to_string();

    let mut args = vec![format!("-Xmx{}M", ctx.memory_mb.max(512))];
    args.extend(ctx.extra_jvm.split_whitespace().map(String::from));
    args.extend(jvm);
    args.push(main_class);
    args.extend(game);
    // Never pass an unresolved placeholder to the game.
    Ok(args.into_iter().filter(|a| !a.contains("${")).collect())
}

pub fn spawn(java: &Path, args: &[String], game_dir: &Path) -> Result<Child> {
    std::fs::create_dir_all(game_dir)?;
    let mut cmd = Command::new(java);
    cmd.args(args)
        .current_dir(game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut cmd);
    cmd.spawn()
        .with_context(|| format!("could not start Java at {}", java.display()))
}
