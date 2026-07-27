use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub hotkey: String,
    pub window_width: f32,
    pub window_height: f32,
    pub debounce_delay_ms: u64,
    pub search_url: String,
    pub search_dirs: Vec<String>,
    pub clipboard_max_entries: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Space".into(),
            window_width: 580.0,
            window_height: 420.0,
            debounce_delay_ms: 150,
            search_url: "https://duckduckgo.com/search?q=%s".into(),
            search_dirs: vec![],
            clipboard_max_entries: 100,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            let old = Self::old_config_path();
            if old.exists() {
                // migrate from old JSON config
                if let Some(cfg) = std::fs::read_to_string(&old)
                    .ok()
                    .and_then(|s| serde_json::from_str::<Config>(&s).ok())
                {
                    cfg.save();
                    let _ = std::fs::remove_file(&old);
                    return cfg;
                }
            }
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_else(|| {
                let cfg = Config::default();
                cfg.save();
                cfg
            })
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Ok(s) = toml::to_string_pretty(self) {
            std::fs::write(&path, s).ok();
        }
    }

    fn config_path() -> PathBuf {
        let mut path = data_dir();
        path.push("config.toml");
        path
    }

    fn old_config_path() -> PathBuf {
        let mut path = data_dir();
        path.push("config.json");
        path
    }
}

fn data_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".element");
        p
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        let mut p = PathBuf::from(profile);
        p.push(".element");
        p
    } else {
        PathBuf::from(".element")
    }
}
