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

pub(crate) fn data_dir() -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_are_valid() {
        let cfg = Config::default();
        assert_eq!(cfg.hotkey, "Alt+Space");
        assert!(cfg.window_width > 0.0);
        assert!(cfg.debounce_delay_ms > 0);
        assert!(cfg.search_url.contains("%s"));
    }

    #[test]
    fn config_toml_round_trip() {
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.hotkey, cfg.hotkey);
        assert!((parsed.window_width - cfg.window_width).abs() < f32::EPSILON);
        assert_eq!(parsed.search_url, cfg.search_url);
    }

    #[test]
    fn config_override_fields() {
        let cfg = Config {
            hotkey: "Ctrl+Shift+F".into(),
            window_width: 800.0,
            search_url: "https://google.com/search?q=%s".into(),
            ..Default::default()
        };
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.hotkey, "Ctrl+Shift+F");
        assert!((parsed.window_width - 800.0).abs() < f32::EPSILON);
        assert_eq!(parsed.search_url, "https://google.com/search?q=%s");
    }

    #[test]
    fn config_json_migration_format() {
        let cfg = Config::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hotkey, cfg.hotkey);
    }

    #[test]
    fn config_data_dir_exists() {
        let dir = data_dir();
        assert!(dir.to_string_lossy().contains(".element"));
    }
}
