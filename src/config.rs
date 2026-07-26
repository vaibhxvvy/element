use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub enum Theme {
    Dark,
    Light,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ElementConfig {
    pub theme: Theme,
    pub window_width: f32,
    pub window_height: f32,
    pub word_wrap: bool,
    pub hotkey: String,
}

impl Default for ElementConfig {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            window_width: 920.0,
            window_height: 660.0,
            word_wrap: false,
            hotkey: "Ctrl+Space".into(),
        }
    }
}

impl ElementConfig {
    fn config_dir() -> PathBuf {
        let mut path = dirs_data_dir();
        path.push("element");
        path
    }

    pub fn config_path() -> PathBuf {
        let mut path = Self::config_dir();
        std::fs::create_dir_all(&path).ok();
        path.push("config.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            std::fs::write(path, json).ok();
        }
    }
}

fn dirs_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var("ELEMENT_DATA_DIR")
        .ok()
        .map(PathBuf::from)
    {
        return dir;
    }
    if let Some(dir) = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
    {
        return dir;
    }
    PathBuf::from(".")
}
