use crate::util::{read_json_file, write_json_file};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// All on-disk locations. Portable mode: put a `portable.txt` next to the executable
/// and everything is stored in `./data` instead of the user profile.
#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn new() -> Paths {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                if dir.join("portable.txt").exists() {
                    return Paths { root: dir.join("data") };
                }
            }
        }
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        Paths { root: base.join("SpiderFlayer") }
    }

    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn runtime(&self) -> PathBuf {
        self.root.join("runtime")
    }
    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }

    /// Instance ids come from the UI, so they are validated before touching the disk.
    pub fn instance_dir(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty()
            || id.len() > 64
            || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!("invalid instance id");
        }
        Ok(self.instances().join(id))
    }
    pub fn game_dir(&self, id: &str) -> Result<PathBuf> {
        Ok(self.instance_dir(id)?.join("minecraft"))
    }
}

impl Default for Paths {
    fn default() -> Self {
        Paths::new()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub curseforge_key: String,
    pub ms_client_id: String,
    pub java_path: String,
    pub default_memory_mb: u32,
    /// File name of a custom background inside the data folder. `None` = the built-in one.
    pub background: Option<String>,
    pub bg_blur: u32,
    pub bg_dim: u32,
    pub glass: u32,
    pub accent: String,
    pub theme: String,
    pub hide_on_launch: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            curseforge_key: String::new(),
            ms_client_id: String::new(),
            java_path: String::new(),
            default_memory_mb: 2048,
            background: None,
            bg_blur: 0,
            bg_dim: 25,
            glass: 60,
            accent: "#8b5cf6".to_string(),
            theme: "oled".to_string(),
            hide_on_launch: true,
        }
    }
}

impl Settings {
    pub fn load(paths: &Paths) -> Settings {
        read_json_file(&paths.settings_file())
    }
    pub fn save(&self, paths: &Paths) -> Result<()> {
        write_json_file(&paths.settings_file(), self)
    }
}
