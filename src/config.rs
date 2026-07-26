use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    pub primary: String,
    pub ink: String,
    pub core: String,
    pub muted: String,
    pub surface: String,
    pub border: String,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            primary: "#6D4AFA".into(),
            ink: "#08060D".into(),
            core: "#FDFAFE".into(),
            muted: "#A7A0BD".into(),
            surface: "#12101A".into(),
            border: "#2A2740".into(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ClipboardConfig {
    pub enabled: bool,
    pub poll_interval_ms: u64,
    pub max_entries: u32,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_ms: 500,
            max_entries: 200,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ElementConfig {
    pub theme_colors: ThemeColors,
    pub window_width: f32,
    pub window_height: f32,
    pub word_wrap: bool,
    pub hotkey: String,
    pub debounce_delay_ms: u64,
    pub clipboard: ClipboardConfig,
}

impl Default for ElementConfig {
    fn default() -> Self {
        Self {
            theme_colors: ThemeColors::default(),
            window_width: 920.0,
            window_height: 660.0,
            word_wrap: false,
            hotkey: "Alt+Space".into(),
            debounce_delay_ms: 200,
            clipboard: ClipboardConfig::default(),
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

    pub fn primary(&self) -> u32 {
        u32::from_str_radix(&self.theme_colors.primary.trim_start_matches('#'), 16)
            .unwrap_or(0x6D4AFA)
    }

    pub fn ink(&self) -> u32 {
        u32::from_str_radix(&self.theme_colors.ink.trim_start_matches('#'), 16)
            .unwrap_or(0x08060D)
    }

    pub fn core(&self) -> u32 {
        u32::from_str_radix(&self.theme_colors.core.trim_start_matches('#'), 16)
            .unwrap_or(0xFDFAFE)
    }

    pub fn muted(&self) -> u32 {
        u32::from_str_radix(&self.theme_colors.muted.trim_start_matches('#'), 16)
            .unwrap_or(0xA7A0BD)
    }

    pub fn surface(&self) -> u32 {
        u32::from_str_radix(&self.theme_colors.surface.trim_start_matches('#'), 16)
            .unwrap_or(0x12101A)
    }

    pub fn border(&self) -> u32 {
        u32::from_str_radix(&self.theme_colors.border.trim_start_matches('#'), 16)
            .unwrap_or(0x2A2740)
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
