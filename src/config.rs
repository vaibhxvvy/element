use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub hotkey: String,
    pub window_width: f32,
    pub window_height: f32,
    pub debounce_delay_ms: u64,
    pub search_url: String,
    pub search_dirs: Vec<String>,
    /// Directories indexed for file search (`file`/`folder` prefixes).
    /// Empty = default to the user home folder.
    pub file_search_dirs: Vec<String>,
    pub clipboard_max_entries: u32,
    /// Launch Element automatically when Windows starts.
    /// Defaults to true; missing in older config.toml files.
    pub autostart: bool,
    /// Accent color as `#rrggbb` hex, used for selection highlights.
    pub accent: String,
    /// Maximum directory depth walked by the file index.
    pub file_index_depth: usize,
    /// Hard cap on indexed file entries.
    pub file_index_entries: usize,
    /// Per-site search shortcuts: `yt cats` → YouTube search. Key is the
    /// prefix, value a URL template with `%s` for the encoded query.
    pub search_prefixes: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: "Alt+Space".into(),
            window_width: 960.0,
            window_height: 420.0,
            debounce_delay_ms: 150,
            search_url: "https://duckduckgo.com/search?q=%s".into(),
            search_dirs: vec![],
            file_search_dirs: vec![],
            clipboard_max_entries: 100,
            autostart: true,
            accent: "#569cd4".into(),
            file_index_depth: 14,
            file_index_entries: 50_000,
            search_prefixes: [
                ("yt", "https://www.youtube.com/results?search_query=%s"),
                ("gh", "https://github.com/search?q=%s"),
                ("w", "https://en.wikipedia.org/w/index.php?search=%s"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
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

/// RegisterHotKey modifier bits (without `MOD_NOREPEAT`).
pub(crate) const MOD_ALT: u32 = 0x0001;
pub(crate) const MOD_CONTROL: u32 = 0x0002;
pub(crate) const MOD_SHIFT: u32 = 0x0004;
pub(crate) const MOD_WIN: u32 = 0x0008;

/// Parse `"Alt+Space"`, `"Ctrl+Shift+F"`, etc. into `(modifiers, virtual_key)`.
pub(crate) fn parse_hotkey(s: &str) -> Option<(u32, u32)> {
    let mut mods = 0u32;
    let mut vk: Option<u32> = None;
    for part in s.split('+').map(str::trim) {
        if part.is_empty() {
            return None;
        }
        match part.to_ascii_lowercase().as_str() {
            "alt" | "menu" => mods |= MOD_ALT,
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "shift" => mods |= MOD_SHIFT,
            "win" | "super" | "meta" => mods |= MOD_WIN,
            "space" => vk = Some(0x20),
            other if other.len() == 1 => {
                let c = other.chars().next()?.to_ascii_uppercase();
                if c.is_ascii_alphanumeric() {
                    vk = Some(c as u32);
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some((mods, vk?))
}

/// Preferred hotkey first, then common launcher alternatives.
pub(crate) fn hotkey_fallback_candidates(preferred: &str) -> Vec<String> {
    let mut out = Vec::new();
    let push = |list: &mut Vec<String>, s: &str| {
        if !list.iter().any(|x| x.eq_ignore_ascii_case(s)) {
            list.push(s.to_string());
        }
    };
    push(&mut out, preferred);
    for c in [
        "Alt+Space",
        "Ctrl+Space",
        "Alt+Shift+Space",
        "Ctrl+Shift+Space",
        "Ctrl+Alt+Space",
    ] {
        push(&mut out, c);
    }
    out
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
        assert!(cfg.autostart);
    }

    #[test]
    fn config_missing_autostart_defaults_true() {
        let toml_str = r#"
hotkey = "Alt+Space"
window_width = 960.0
window_height = 420.0
debounce_delay_ms = 150
search_url = "https://duckduckgo.com/search?q=%s"
search_dirs = []
clipboard_max_entries = 100
"#;
        let parsed: Config = toml::from_str(toml_str).unwrap();
        assert!(parsed.autostart);
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

    #[test]
    fn search_prefixes_round_trip() {
        let cfg = Config::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            parsed.search_prefixes.get("yt").map(String::as_str),
            Some("https://www.youtube.com/results?search_query=%s")
        );
        assert_eq!(
            parsed.search_prefixes.get("gh").map(String::as_str),
            Some("https://github.com/search?q=%s")
        );
    }

    #[test]
    fn parse_hotkey_alt_space() {
        let (mods, vk) = parse_hotkey("Alt+Space").unwrap();
        assert_eq!(mods, MOD_ALT);
        assert_eq!(vk, 0x20);
    }

    #[test]
    fn parse_hotkey_ctrl_shift_f() {
        let (mods, vk) = parse_hotkey("Ctrl+Shift+F").unwrap();
        assert_eq!(mods, MOD_CONTROL | MOD_SHIFT);
        assert_eq!(vk, b'F' as u32);
    }

    #[test]
    fn hotkey_fallback_keeps_preferred_first() {
        let list = hotkey_fallback_candidates("Ctrl+Space");
        assert_eq!(list[0], "Ctrl+Space");
        assert!(list.iter().any(|s| s == "Alt+Space"));
    }
}
