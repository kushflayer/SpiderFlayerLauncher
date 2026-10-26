#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use spiderflayer_core::config::{Paths, Settings};
use spiderflayer_core::instance::{self, ContentFile, Instance};
use spiderflayer_core::util::{safe_file_name, Progress};
use spiderflayer_core::{auth, launch, loaders, modpack, mods, mojang};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

struct AppState {
    paths: Paths,
    settings: Mutex<Settings>,
    /// instance id -> process id
    running: Mutex<HashMap<String, u32>>,
    /// Set to abort a Microsoft sign-in that is waiting for the browser.
    ms_cancel: Arc<AtomicBool>,
}

type Res<T> = Result<T, String>;

fn err<T>(r: anyhow::Result<T>) -> Res<T> {
    r.map_err(|e| format!("{e:#}"))
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

fn emit_progress(app: &AppHandle) -> impl Fn(Progress) + Sync + '_ {
    move |p: Progress| {
        let _ = app.emit("progress", p);
    }
}

// ---- settings & background --------------------------------------------------------------

#[tauri::command(async)]
fn get_settings(state: State<AppState>) -> Res<Settings> {
    Ok(state.settings.lock().unwrap().clone())
}

#[tauri::command(async)]
fn save_settings(state: State<AppState>, settings: Settings) -> Res<()> {
    err(settings.save(&state.paths))?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[derive(Serialize)]
struct Background {
    mime: String,
    data: String,
}

#[tauri::command(async)]
fn get_background(state: State<AppState>) -> Res<Option<Background>> {
    let name = match state.settings.lock().unwrap().background.clone() {
        Some(n) => n,
        None => return Ok(None),
    };
    let file = err(safe_file_name(&name))?;
    let bytes = match fs::read(state.paths.root.join(&file)) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let mime = match file.rsplit('.').next().unwrap_or("") {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/jpeg",
    };
    Ok(Some(Background {
        mime: mime.to_string(),
        data: b64().encode(bytes),
    }))
}

#[tauri::command(async)]
fn set_background(state: State<AppState>, ext: String, data: String) -> Res<()> {
    let ext = match ext.as_str() {
        "png" | "webp" | "gif" | "jpg" | "jpeg" => ext,
        _ => return Err("Use a PNG, JPG, WebP or GIF image".into()),
    };
    let bytes = b64().decode(data).map_err(|e| e.to_string())?;
    if bytes.len() > 40 * 1024 * 1024 {
        return Err("That image is too large (40 MB max)".into());
    }
    let name = format!("background.{ext}");
    fs::create_dir_all(&state.paths.root).map_err(|e| e.to_string())?;
    fs::write(state.paths.root.join(&name), bytes).map_err(|e| e.to_string())?;
    let mut s = state.settings.lock().unwrap();
    s.background = Some(name);
    err(s.save(&state.paths))
}

#[tauri::command(async)]
fn clear_background(state: State<AppState>) -> Res<()> {
    let mut s = state.settings.lock().unwrap();
    if let Some(name) = s.background.take() {
        if let Ok(f) = safe_file_name(&name) {
            let _ = fs::remove_file(state.paths.root.join(f));
        }
    }
    err(s.save(&state.paths))
}

// ---- versions & instances ---------------------------------------------------------------

#[tauri::command(async)]
fn mc_versions() -> Res<Vec<Value>> {
    err(mojang::list_versions())
}

#[tauri::command(async)]
fn loader_versions(loader: String, mc: String) -> Res<Vec<String>> {
    err(loaders::versions(&loader, &mc))
}

#[tauri::command(async)]
fn list_instances(state: State<AppState>) -> Res<Vec<Instance>> {
    Ok(instance::list(&state.paths))
}

#[tauri::command(async)]
fn create_instance(
    state: State<AppState>,
    name: String,
    mc: String,
    loader: String,
    loader_version: String,
) -> Res<Instance> {
    err(instance::create(&state.paths, &name, &mc, &loader, &loader_version))
}

#[tauri::command(async)]
fn update_instance(state: State<AppState>, inst: Instance) -> Res<()> {
    // Only the editable fields are taken from the UI.
    let mut cur = err(instance::get(&state.paths, &inst.id))?;
    cur.name = inst.name;
    cur.memory_mb = inst.memory_mb;
    cur.java_args = inst.java_args;
    err(instance::save(&state.paths, &cur))
}

#[tauri::command(async)]
fn delete_instance(state: State<AppState>, id: String) -> Res<()> {
    err(instance::delete(&state.paths, &id))
}

#[tauri::command(async)]
fn list_content(state: State<AppState>, id: String, kind: String) -> Res<Vec<ContentFile>> {
    err(instance::content(&state.paths, &id, &kind))
}

#[tauri::command(async)]
fn toggle_content(
    state: State<AppState>,
    id: String,
    kind: String,
    name: String,
    enabled: bool,
) -> Res<()> {
    err(instance::set_enabled(&state.paths, &id, &kind, &name, enabled))
}

#[tauri::command(async)]
fn remove_content(state: State<AppState>, id: String, kind: String, name: String) -> Res<()> {
    err(instance::remove(&state.paths, &id, &kind, &name))
}

fn open_path(target: &str) {
    #[cfg(windows)]
    let mut cmd = Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let mut cmd = Command::new("xdg-open");
    cmd.arg(target);
    spiderflayer_core::util::hide_window(&mut cmd);
    let _ = cmd.spawn();
}

#[tauri::command(async)]
fn open_instance_folder(state: State<AppState>, id: String) -> Res<()> {
    let dir = err(state.paths.game_dir(&id))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    open_path(&dir.to_string_lossy());
    Ok(())
}

fn open_external(url: &str) {
    #[cfg(windows)]
    {
        // rundll32 hands the URL straight to the default browser; no cmd.exe quoting rules.
        let mut cmd = Command::new("rundll32");
        cmd.args(["url.dll,FileProtocolHandler", url]);
        spiderflayer_core::util::hide_window(&mut cmd);
        let _ = cmd.spawn();
    }
    #[cfg(not(windows))]
    open_path(url);
}

#[tauri::command(async)]
fn open_url(url: String) -> Res<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only web links can be opened".into());
    }
    open_external(&url);
    Ok(())
}

// ---- accounts ---------------------------------------------------------------------------

#[tauri::command(async)]
fn accounts_list(state: State<AppState>) -> Res<Value> {
    Ok(auth::public(&auth::load(&state.paths)))
}

#[tauri::command(async)]
fn add_offline(state: State<AppState>, name: String) -> Res<Value> {
    err(auth::add_offline(&state.paths, &name)).map(|s| auth::public(&s))
}

#[tauri::command(async)]
fn select_account(state: State<AppState>, id: String) -> Res<Value> {
    err(auth::select(&state.paths, &id)).map(|s| auth::public(&s))
}

#[tauri::command(async)]
fn remove_account(state: State<AppState>, id: String) -> Res<Value> {
    err(auth::remove(&state.paths, &id)).map(|s| auth::public(&s))
}

#[tauri::command(async)]
fn ms_configured(state: State<AppState>) -> Res<bool> {
    let id = state.settings.lock().unwrap().ms_client_id.clone();
    Ok(!auth::client_id(&id).is_empty())
}

#[tauri::command(async)]
fn ms_cancel(state: State<AppState>) -> Res<()> {
    state.ms_cancel.store(true, Ordering::Relaxed);
    Ok(())
}

fn open_login_window(app: &AppHandle, url: &str, cancel: Arc<AtomicBool>) -> Result<(), String> {
    if let Some(old) = app.get_webview_window("msauth") {
        let _ = old.close();
    }
    let parsed: tauri::Url = url.parse().map_err(|e| format!("bad sign-in url: {e}"))?;
    let win = tauri::WebviewWindowBuilder::new(app, "msauth", tauri::WebviewUrl::External(parsed))
        .title("Sign in to Microsoft")
        .inner_size(520.0, 720.0)
        .center()
        .build()
        .map_err(|e| e.to_string())?;
    // Closing the login window by hand abandons the sign-in.
    win.on_window_event(move |ev| {
        if let tauri::WindowEvent::Destroyed = ev {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    Ok(())
}

/// One-click Microsoft sign-in: open Microsoft's page, wait for the redirect, done.
/// `in_app` shows the page in a launcher window instead of the default browser.
#[tauri::command(async)]
fn ms_login(app: AppHandle, state: State<AppState>, in_app: bool) -> Res<Value> {
    let id = state.settings.lock().unwrap().ms_client_id.clone();
    let client_id = auth::client_id(&id);
    state.ms_cancel.store(false, Ordering::Relaxed);
    let (url, pending) = err(auth::ms_prepare(&client_id))?;
    if in_app {
        open_login_window(&app, &url, state.ms_cancel.clone())?;
    } else {
        open_external(&url);
    }
    let result = auth::ms_complete(&state.paths, pending, &state.ms_cancel, 300);
    // Closing the window fires its Destroyed event; make sure that cannot cancel the next try.
    if let Some(w) = app.get_webview_window("msauth") {
        let _ = w.close();
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    state.ms_cancel.store(false, Ordering::Relaxed);
    err(result)?;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
    Ok(auth::public(&auth::load(&state.paths)))
}

// ---- mod browser ------------------------------------------------------------------------

#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
fn search(
    state: State<AppState>,
    source: String,
    query: String,
    kind: String,
    mc: String,
    loader: String,
    sort: String,
    offset: u32,
) -> Res<Value> {
    let key = state.settings.lock().unwrap().curseforge_key.clone();
    err(mods::search(&source, &key, &query, &kind, &mc, &loader, &sort, offset))
}

#[tauri::command(async)]
fn install_content(
    state: State<AppState>,
    source: String,
    project: String,
    kind: String,
    instance_id: String,
) -> Res<Vec<String>> {
    let key = state.settings.lock().unwrap().curseforge_key.clone();
    let inst = err(instance::get(&state.paths, &instance_id))?;
    let game_dir = err(state.paths.game_dir(&inst.id))?;
    err(mods::install(
        &source,
        &key,
        &project,
        &kind,
        &inst.mc_version,
        &inst.loader,
        &game_dir,
    ))
}

#[tauri::command(async)]
fn project_details(state: State<AppState>, source: String, id: String) -> Res<Value> {
    let key = state.settings.lock().unwrap().curseforge_key.clone();
    err(mods::project_details(&source, &key, &id))
}

#[tauri::command(async)]
fn project_versions(
    state: State<AppState>,
    source: String,
    project: String,
    kind: String,
    mc: String,
    loader: String,
) -> Res<Value> {
    let key = state.settings.lock().unwrap().curseforge_key.clone();
    err(mods::versions(&source, &key, &project, &kind, &mc, &loader))
}

#[tauri::command(async)]
fn install_version_content(
    state: State<AppState>,
    source: String,
    project: String,
    version_id: String,
    kind: String,
    instance_id: String,
) -> Res<Vec<String>> {
    let key = state.settings.lock().unwrap().curseforge_key.clone();
    let inst = err(instance::get(&state.paths, &instance_id))?;
    let game_dir = err(state.paths.game_dir(&inst.id))?;
    err(mods::install_version(
        &source,
        &key,
        &project,
        &version_id,
        &kind,
        &inst.mc_version,
        &inst.loader,
        &game_dir,
    ))
}

#[tauri::command(async)]
fn install_modpack_version(
    app: AppHandle,
    state: State<AppState>,
    version_id: String,
) -> Res<Instance> {
    let rep = emit_progress(&app);
    err(modpack::install_from_modrinth_version(
        &state.paths,
        &version_id,
        &rep,
    ))
}

#[tauri::command(async)]
fn install_modpack(app: AppHandle, state: State<AppState>, project: String) -> Res<Instance> {
    let rep = emit_progress(&app);
    err(modpack::install_from_modrinth(&state.paths, &project, &rep))
}

#[tauri::command(async)]
fn import_mrpack(app: AppHandle, state: State<AppState>, data: String) -> Res<Instance> {
    let bytes = b64().decode(data).map_err(|e| e.to_string())?;
    fs::create_dir_all(state.paths.tmp()).map_err(|e| e.to_string())?;
    let tmp = state.paths.tmp().join("import.mrpack");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    let rep = emit_progress(&app);
    let r = modpack::install_mrpack(&state.paths, &tmp, &rep);
    let _ = fs::remove_file(&tmp);
    err(r)
}

// ---- launching --------------------------------------------------------------------------

#[tauri::command(async)]
fn running(state: State<AppState>) -> Res<Vec<String>> {
    Ok(state.running.lock().unwrap().keys().cloned().collect())
}

#[tauri::command(async)]
fn kill_game(state: State<AppState>, id: String) -> Res<()> {
    let pid = state.running.lock().unwrap().get(&id).copied();
    if let Some(pid) = pid {
        #[cfg(windows)]
        let mut cmd = {
            let mut c = Command::new("taskkill");
            c.args(["/PID", &pid.to_string(), "/F"]);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = Command::new("kill");
            c.args(["-9", &pid.to_string()]);
            c
        };
        spiderflayer_core::util::hide_window(&mut cmd);
        let _ = cmd.output();
    }
    Ok(())
}

fn emit_log(app: &AppHandle, id: &str, line: String) {
    let _ = app.emit("log", json!({ "id": id, "line": line }));
}

#[tauri::command(async)]
fn launch_game(app: AppHandle, state: State<AppState>, id: String) -> Res<()> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err("This instance is already running".into());
    }
    let settings = state.settings.lock().unwrap().clone();
    let paths = state.paths.clone();
    let rep = emit_progress(&app);

    let mut inst = err(instance::get(&paths, &id))?;
    let acct = err(auth::ensure_fresh(&paths, &settings.ms_client_id))?;
    let java = err(instance::prepare(&paths, &mut inst, &settings, &rep))?;
    inst.last_played = spiderflayer_core::util::now_secs();
    err(instance::save(&paths, &inst))?;

    let game_dir = err(paths.game_dir(&inst.id))?;
    let memory = if inst.memory_mb > 0 {
        inst.memory_mb
    } else {
        settings.default_memory_mb
    };
    let (token, user_type) = if acct.kind == "microsoft" {
        (acct.access_token.as_str(), "msa")
    } else {
        ("0", "legacy")
    };
    let args = err(launch::build(
        &paths,
        &launch::LaunchCtx {
            version_id: &inst.version_id,
            player: &acct.name,
            uuid: &acct.id,
            token,
            user_type,
            game_dir: &game_dir,
            memory_mb: memory,
            extra_jvm: &inst.java_args,
        },
    ))?;
    rep(Progress::msg("launch", "Starting Minecraft"));
    let mut child = err(launch::spawn(&java, &args, &game_dir))?;
    let pid = child.id();
    state.running.lock().unwrap().insert(id.clone(), pid);
    let _ = app.emit("game-started", json!({ "id": id }));

    for stream in [
        child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let app2 = app.clone();
        let id2 = id.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                emit_log(&app2, &id2, line);
            }
        });
    }

    if settings.hide_on_launch {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.hide();
        }
    }
    let app3 = app.clone();
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code());
        app3.state::<AppState>().running.lock().unwrap().remove(&id);
        if let Some(w) = app3.get_webview_window("main") {
            let _ = w.show();
            let _ = w.set_focus();
        }
        let _ = app3.emit("game-exit", json!({ "id": id, "code": code }));
    });
    Ok(())
}

fn main() {
    let paths = Paths::new();
    let _ = fs::create_dir_all(&paths.root);
    let settings = Settings::load(&paths);
    tauri::Builder::default()
        .manage(AppState {
            paths,
            settings: Mutex::new(settings),
            running: Mutex::new(HashMap::new()),
            ms_cancel: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_background,
            set_background,
            clear_background,
            mc_versions,
            loader_versions,
            list_instances,
            create_instance,
            update_instance,
            delete_instance,
            list_content,
            toggle_content,
            remove_content,
            open_instance_folder,
            open_url,
            accounts_list,
            add_offline,
            select_account,
            remove_account,
            ms_configured,
            ms_login,
            ms_cancel,
            project_details,
            project_versions,
            install_version_content,
            install_modpack_version,
            search,
            install_content,
            install_modpack,
            import_mrpack,
            running,
            kill_game,
            launch_game,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start Spiderflayer Launcher");
}
