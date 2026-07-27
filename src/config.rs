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
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
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
        if let Ok(s) = serde_json::to_string_pretty(self) {
            std::fs::write(&path, s).ok();
        }
    }

    fn config_path() -> PathBuf {
        let mut path = if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
        } else if let Ok(profile) = std::env::var("USERPROFILE") {
            PathBuf::from(profile)
        } else {
            PathBuf::from(".")
        };
        path.push(".element");
        path.push("config.json");
        path
    }
}
