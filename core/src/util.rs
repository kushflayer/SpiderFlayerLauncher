use anyhow::{anyhow, bail, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use sha1::{Digest, Sha1};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const UA: &str = "SpiderFlayer-Launcher/0.1.0";

static AGENT: OnceLock<ureq::Agent> = OnceLock::new();

/// One shared HTTP agent (connection pooling, no async runtime needed).
pub fn http() -> &'static ureq::Agent {
    AGENT.get_or_init(|| {
        ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(15))
            .timeout_read(Duration::from_secs(60))
            .user_agent(UA)
            .build()
    })
}

pub fn http_err(e: ureq::Error) -> anyhow::Error {
    match e {
        ureq::Error::Status(code, resp) => {
            let url = resp.get_url().to_string();
            let body: String = resp.into_string().unwrap_or_default().chars().take(300).collect();
            if body.is_empty() {
                anyhow!("HTTP {code} from {url}")
            } else {
                anyhow!("HTTP {code} from {url}: {body}")
            }
        }
        other => anyhow!("network error: {other}"),
    }
}

pub fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
    let resp = http().get(url).call().map_err(http_err)?;
    resp.into_json::<T>()
        .with_context(|| format!("invalid JSON from {url}"))
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

pub fn current_arch() -> &'static str {
    if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

pub fn cp_sep() -> &'static str {
    if cfg!(windows) {
        ";"
    } else {
        ":"
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn file_sha1(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

fn try_download(url: &str, tmp: &Path, sha1: Option<&str>) -> Result<()> {
    let resp = http().get(url).call().map_err(http_err)?;
    let mut reader = resp.into_reader();
    let mut file = fs::File::create(tmp)?;
    let mut hasher = Sha1::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
    }
    file.flush()?;
    if let Some(expected) = sha1 {
        let got = hex(&hasher.finalize());
        if !got.eq_ignore_ascii_case(expected) {
            bail!("checksum mismatch for {url}");
        }
    }
    Ok(())
}

/// Download `url` to `dest` unless a valid copy is already there.
/// With a hash the existing file is verified; without one, any non-empty file is trusted.
pub fn download(url: &str, dest: &Path, sha1: Option<&str>) -> Result<()> {
    if dest.exists() {
        match sha1 {
            Some(h) => {
                if file_sha1(dest)
                    .map(|got| got.eq_ignore_ascii_case(h))
                    .unwrap_or(false)
                {
                    return Ok(());
                }
            }
            None => {
                if fs::metadata(dest).map(|m| m.len() > 0).unwrap_or(false) {
                    return Ok(());
                }
            }
        }
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp_name = dest.as_os_str().to_owned();
    tmp_name.push(".part");
    let tmp = PathBuf::from(tmp_name);

    let mut last_err = anyhow!("download failed: {url}");
    for attempt in 0..3u64 {
        match try_download(url, &tmp, sha1) {
            Ok(()) => {
                fs::rename(&tmp, dest)?;
                return Ok(());
            }
            Err(e) => {
                last_err = e;
                std::thread::sleep(Duration::from_millis(400 * (attempt + 1)));
            }
        }
    }
    let _ = fs::remove_file(&tmp);
    Err(last_err)
}

/// Run `f` over `items` on up to `threads` worker threads; stops at the first error.
pub fn par_each<T, F>(items: &[T], threads: usize, f: F) -> Result<()>
where
    T: Sync,
    F: Fn(&T) -> Result<()> + Sync,
{
    let next = AtomicUsize::new(0);
    let failure: Mutex<Option<anyhow::Error>> = Mutex::new(None);
    let workers = threads.min(items.len()).max(1);
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                if failure.lock().unwrap().is_some() {
                    break;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                if let Err(e) = f(&items[i]) {
                    *failure.lock().unwrap() = Some(e);
                    break;
                }
            });
        }
    });
    match failure.into_inner().unwrap() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Reject absolute paths and `..` (zip-slip / path traversal).
pub fn safe_rel(p: &str) -> Result<PathBuf> {
    let path = Path::new(p);
    for c in path.components() {
        if !matches!(c, Component::Normal(_)) {
            bail!("unsafe path in archive: {p}");
        }
    }
    Ok(path.to_path_buf())
}

/// A bare, safe file name (no directories).
pub fn safe_file_name(name: &str) -> Result<String> {
    let n = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if n.is_empty() || n == ".." || n != name {
        bail!("invalid file name: {name}");
    }
    Ok(n.to_string())
}

pub fn read_json_file<T: DeserializeOwned + Default>(path: &Path) -> T {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

// ---- progress reporting -------------------------------------------------

#[derive(Serialize, Clone, Debug)]
pub struct Progress {
    pub stage: String,
    pub done: u64,
    pub total: u64,
    pub message: String,
}

impl Progress {
    pub fn new(stage: &str, done: u64, total: u64, message: &str) -> Self {
        Progress {
            stage: stage.to_string(),
            done,
            total,
            message: message.to_string(),
        }
    }
    pub fn msg(stage: &str, message: &str) -> Self {
        Progress::new(stage, 0, 0, message)
    }
}

pub type Reporter<'a> = &'a (dyn Fn(Progress) + Sync);

/// Keep child processes from flashing a console window on Windows.
pub fn hide_window(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}
