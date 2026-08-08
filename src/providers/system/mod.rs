//! System commands provider — `shutdown`, `restart`, `sleep`, `lock`, plus
//! everyday quick actions: `volume`, `mute`, `screen off`, `timer`, `password`
//! and `screenshot`.
//!
//! Matches the command name at the start of the query (word boundary), so
//! `lock` works and `locksmith` does not. Shutdown/restart shell out to
//! `shutdown.exe`; everything else uses Win32 via [`crate::platform`].

use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider, SearchResult};
use image::ImageEncoder;

/// Encode RGBA pixels as a PNG file (memory buffer) for clipboard use.
fn encode_png(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(rgba, w, h, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("could not encode screenshot: {e}"))?;
    Ok(out.into_inner())
}

/// Save an already-encoded PNG as a timestamped file into `Pictures\Screenshots`
/// (falling back to the data dir), so screenshots are findable even when
/// pasting into a clipboard is not. Takes the encoded PNG so the frame is
/// only compressed once (encoding a full screen twice is the slow part).
fn save_screenshot_png(png: &[u8]) -> Result<String, String> {
    let base = std::env::var_os("USERPROFILE")
        .map(|p| {
            std::path::PathBuf::from(p)
                .join("Pictures")
                .join("Screenshots")
        })
        .filter(|p| std::fs::create_dir_all(p).is_ok())
        .unwrap_or_else(|| {
            let dir = crate::config::data_dir().join("screenshots");
            let _ = std::fs::create_dir_all(&dir);
            dir
        });
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = base.join(format!("element-{stamp}.png"));
    std::fs::write(&path, png).map_err(|e| format!("could not save screenshot: {e}"))?;
    Ok(format!("Screenshot saved — {}", path.display()))
}

/// (query keyword, display title, base subtitle, canonical action key)
const COMMANDS: &[(&str, &str, &str, &str)] = &[
    ("shutdown", "Shut down", "Turn the computer off", "shutdown"),
    ("restart", "Restart", "Restart the computer", "restart"),
    ("reboot", "Restart", "Restart the computer", "restart"),
    ("sleep", "Sleep", "Put the computer to sleep", "sleep"),
    ("lock", "Lock", "Lock the computer", "lock"),
    (
        "volume",
        "Volume",
        "Set the volume, e.g. volume 40",
        "volume",
    ),
    ("mute", "Mute", "Turn the volume down to 0", "mute"),
    (
        "screen off",
        "Screen off",
        "Turn the display off",
        "screenoff",
    ),
    (
        "timer",
        "Timer",
        "Ping after N minutes, e.g. timer 10 · timer 5m · timer 30s",
        "timer",
    ),
    (
        "password",
        "Strong password",
        "Random secure password to the clipboard (password 24 for length)",
        "password",
    ),
    (
        "screenshot",
        "Screenshot",
        "Capture the whole screen to the clipboard",
        "screenshot",
    ),
];

fn parse(query: &str) -> Option<&'static (&'static str, &'static str, &'static str, &'static str)> {
    let q = query.trim().to_lowercase();
    COMMANDS
        .iter()
        .find(|(keyword, _, _, _)| q == *keyword || q.starts_with(&format!("{} ", keyword)))
}

/// The text after `keyword` in `query` (trimmed), or `""` when absent.
fn arg_after<'a>(query: &'a str, keyword: &str) -> &'a str {
    query
        .trim()
        .get(keyword.len()..)
        .map(str::trim)
        .unwrap_or("")
}

/// Parse a timer duration: bare number = minutes, `90s` seconds, `5m` minutes,
/// `1h` hours. Returns total seconds (1 min .. 24 h).
fn parse_timer_seconds(arg: &str) -> Option<u32> {
    let a = arg.trim().to_lowercase();
    let (digits, mult) = if let Some(s) = a.strip_suffix('h') {
        (s, 3600u64)
    } else if let Some(s) = a.strip_suffix('m') {
        (s, 60)
    } else if let Some(s) = a.strip_suffix('s') {
        (s, 1)
    } else {
        (a.as_str(), 60)
    };
    let n: u64 = digits.trim().parse().ok()?;
    let secs = n.checked_mul(mult)?;
    if secs == 0 || secs > 24 * 3600 {
        return None;
    }
    Some(secs as u32)
}

fn format_duration(total_secs: u32) -> String {
    let (m, s) = (total_secs / 60, total_secs % 60);
    if m > 0 && s > 0 {
        format!("{m} min {s} sec")
    } else if m > 0 {
        format!("{m} min")
    } else {
        format!("{s} sec")
    }
}

pub struct SystemProvider;

impl SearchProvider for SystemProvider {
    fn id(&self) -> &'static str {
        "system"
    }

    fn priority(&self) -> i32 {
        9
    }

    fn should_run(&self, query: &str) -> bool {
        parse(query).is_some()
    }

    fn search(&self, _ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let Some((keyword, title, base_subtitle, canonical)) = parse(query) else {
            return Vec::new();
        };

        let (subtitle, action) = match *keyword {
            "volume" => {
                let arg = arg_after(query, "volume");
                if arg.is_empty() {
                    let current = crate::platform::system_volume()
                        .map(|v| format!("Current volume: {v}%"))
                        .unwrap_or_else(|_| "Set the volume, e.g. volume 40".into());
                    (current, "volume:show".into())
                } else {
                    let Ok(level) = arg.parse::<u32>() else {
                        return Vec::new();
                    };
                    if level > 100 {
                        return Vec::new();
                    }
                    (format!("Set volume to {level}%"), format!("volume:{level}"))
                }
            }
            "timer" => {
                let Some(secs) = parse_timer_seconds(arg_after(query, "timer")) else {
                    return Vec::new();
                };
                (
                    format!("Ping me in {}", format_duration(secs)),
                    format!("timer:{secs}"),
                )
            }
            "password" => {
                let n = arg_after(query, "password").parse::<usize>().unwrap_or(16);
                (
                    format!("{n}-character random password → clipboard"),
                    format!("password:{n}"),
                )
            }
            "mute" => ("Turn the volume down to 0".into(), "mute".into()),
            "screen off" => ("Turn the display off".into(), "screenoff".into()),
            "screenshot" => ("Whole screen → clipboard".into(), "screenshot".into()),
            _ => ((*base_subtitle).into(), (*canonical).to_string()),
        };

        vec![SearchResult {
            title: (*title).into(),
            subtitle,
            kind: if action.starts_with("password:") {
                "password".into()
            } else {
                "system".into()
            },
            provider_id: "system".into(),
            action,
            icon_rgba: None,
            score: 300.0,
        }]
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        let err = |msg: String| ElementError::Other(msg);
        match result.action.as_str() {
            "lock" => crate::platform::lock_workstation().map_err(ElementError::Other),
            "sleep" => crate::platform::suspend_system().map_err(ElementError::Other),
            "shutdown" => run_shutdown("/s"),
            "restart" => run_shutdown("/r"),
            "mute" => crate::platform::set_system_volume(0).map_err(ElementError::Other),
            "screenoff" => crate::platform::turn_screen_off().map_err(ElementError::Other),
            "screenshot" => {
                let (rgba, w, h) =
                    crate::platform::capture_screen_bitmap().map_err(ElementError::Other)?;
                let dib = crate::platform::rgba_to_dib(&rgba, w, h);
                let png = encode_png(&rgba, w, h).map_err(err)?;
                crate::platform::set_clipboard_screenshot(&dib, &png)
                    .map_err(ElementError::Other)?;
                save_screenshot_png(&png).map(|_| ()).map_err(err)
            }
            "volume:show" => Ok(()),
            action if action.starts_with("volume:") => {
                let level = action["volume:".len()..].parse::<u32>().unwrap_or(0);
                crate::platform::set_system_volume(level).map_err(ElementError::Other)
            }
            action if action.starts_with("timer:") => {
                let secs = action["timer:".len()..].parse::<u32>().unwrap_or(0);
                crate::platform::start_timer(secs).map_err(err)
            }
            action if action.starts_with("password:") => {
                let len = action["password:".len()..].parse::<usize>().unwrap_or(16);
                let password = crate::platform::generate_password(len).map_err(err)?;
                crate::platform::set_clipboard_text(&password).map_err(err)
            }
            other => Err(ElementError::Other(format!(
                "unknown system command: {}",
                other
            ))),
        }
    }
}

fn run_shutdown(flag: &str) -> Result<(), ElementError> {
    std::process::Command::new("shutdown.exe")
        .args([flag, "/t", "0"])
        .spawn()
        .map(|_| ())
        .map_err(ElementError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> SearchContext<'static> {
        static CONFIG: std::sync::OnceLock<crate::config::Config> = std::sync::OnceLock::new();
        static DB: std::sync::OnceLock<crate::database::Database> = std::sync::OnceLock::new();
        SearchContext {
            config: CONFIG.get_or_init(crate::config::Config::default),
            db: DB.get_or_init(crate::database::Database::new_in_memory),
        }
    }

    #[test]
    fn matches_command_words_only() {
        let provider = SystemProvider;
        assert!(provider.should_run("lock"));
        assert!(provider.should_run("LOCK"));
        assert!(provider.should_run("restart"));
        assert!(provider.should_run("shutdown now"));
        assert!(provider.should_run("volume 40"));
        assert!(provider.should_run("screen off"));
        assert!(!provider.should_run("locksmith"));
        assert!(!provider.should_run("shut"));
        assert!(!provider.should_run(""));
    }

    #[test]
    fn search_returns_matching_command() {
        let provider = SystemProvider;
        let results = provider.search(&ctx(), "lock");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Lock");
        assert_eq!(results[0].action, "lock");
        assert_eq!(results[0].score, 300.0);

        // "reboot" maps to the restart action.
        let results = provider.search(&ctx(), "reboot");
        assert_eq!(results[0].action, "restart");
    }

    #[test]
    fn volume_needs_a_valid_level() {
        let provider = SystemProvider;
        let results = provider.search(&ctx(), "volume 40");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "volume:40");
        assert!(results[0].subtitle.contains("40%"));
        // bare "volume" shows the current level
        let results = provider.search(&ctx(), "volume");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "volume:show");
        // bad levels produce no result
        assert!(provider.search(&ctx(), "volume loud").is_empty());
        assert!(provider.search(&ctx(), "volume 101").is_empty());
    }

    #[test]
    fn mute_is_static() {
        let provider = SystemProvider;
        let results = provider.search(&ctx(), "mute");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "mute");
    }

    #[test]
    fn timer_parses_durations() {
        // bare number = minutes
        assert_eq!(parse_timer_seconds("10"), Some(600));
        assert_eq!(parse_timer_seconds("5m"), Some(300));
        assert_eq!(parse_timer_seconds("30s"), Some(30));
        assert_eq!(parse_timer_seconds("1h"), Some(3600));
        assert_eq!(parse_timer_seconds("2H"), Some(7200));
        assert_eq!(parse_timer_seconds("1 min"), None);
        assert_eq!(parse_timer_seconds("0"), None);
        assert_eq!(parse_timer_seconds(""), None);
        assert_eq!(parse_timer_seconds("30s"), Some(30));
        // 25 hours is over the 24 h cap
        assert_eq!(parse_timer_seconds("25h"), None);
        assert!(parse_timer_seconds("100000000000000000000m").is_none());
    }

    #[test]
    fn timer_search_builds_action() {
        let provider = SystemProvider;
        let results = provider.search(&ctx(), "timer 10");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "timer:600");
        assert!(results[0].subtitle.contains("10 min"));
        // bare "timer" (no duration) produces no result
        assert!(provider.search(&ctx(), "timer").is_empty());
    }

    #[test]
    fn password_defaults_and_length() {
        let provider = SystemProvider;
        let results = provider.search(&ctx(), "password");
        assert_eq!(results[0].action, "password:16");
        let results = provider.search(&ctx(), "password 24");
        assert_eq!(results[0].action, "password:24");
        let results = provider.search(&ctx(), "password notanumber");
        assert_eq!(results[0].action, "password:16");
    }

    #[test]
    fn screenshot_and_screen_off() {
        let provider = SystemProvider;
        let results = provider.search(&ctx(), "screenshot");
        assert_eq!(results[0].action, "screenshot");
        let results = provider.search(&ctx(), "screen off");
        assert_eq!(results[0].action, "screenoff");
    }

    #[test]
    fn generated_password_shape() {
        let pw = crate::platform::generate_password(16).unwrap();
        assert_eq!(pw.chars().count(), 16);
        assert!(pw.chars().all(|c| c.is_ascii() && !c.is_whitespace()));
        // lengths are clamped to the 8-64 range
        assert_eq!(crate::platform::generate_password(3).unwrap().len(), 8);
        assert_eq!(crate::platform::generate_password(200).unwrap().len(), 64);
        // two draws differ (astronomically improbable to collide)
        assert_ne!(
            crate::platform::generate_password(16).unwrap(),
            crate::platform::generate_password(16).unwrap()
        );
    }
}
