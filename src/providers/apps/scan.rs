//! Start Menu scanning — walks the real directories, resolves each `.lnk`
//! through the icon pipeline, dedups by executable path.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::providers::icon::icon_cache_dir;

use super::icons::{cached_icon, resolve_shortcut};
use super::{is_executable_path, InstalledApp};

/// Walk the given directories (plus the system Start Menu folders) and index
/// every `.lnk` whose target is an existing `.exe`. Keeps one shortcut per
/// case-insensitive executable path, sorted by name.
pub(crate) fn scan_apps(search_dirs: &[String]) -> Vec<InstalledApp> {
    let mut apps = Vec::new();
    #[cfg(target_os = "windows")]
    {
        let mut app_dirs = vec![
            std::env::var("ProgramData")
                .map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
            std::env::var("APPDATA")
                .map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
        ];
        app_dirs.extend(search_dirs.iter().cloned().map(Ok));
        let cache_dir = icon_cache_dir();
        let mut seen_dirs = HashSet::new();
        let mut seen_executables = HashSet::new();
        for dir in app_dirs.into_iter().filter_map(Result::ok) {
            if !seen_dirs.insert(dir.clone()) {
                continue;
            }
            for entry in walkdir::WalkDir::new(&dir)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.extension().map(|e| e == "lnk").unwrap_or(false) {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    if !name.is_empty() && !name.starts_with('.') {
                        if let Some(shortcut) = resolve_shortcut(path) {
                            let executable_path = PathBuf::from(&shortcut.target_path);
                            if is_executable_path(&executable_path) && executable_path.is_file() {
                                if !seen_executables
                                    .insert(shortcut.target_path.to_ascii_lowercase())
                                {
                                    continue;
                                }
                                let icon_rgba = cached_icon(&shortcut, &cache_dir);
                                apps.push(InstalledApp {
                                    name,
                                    executable_path: shortcut.target_path,
                                    icon_rgba,
                                });
                            }
                        }
                    }
                }
            }
        }
        apps.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.executable_path.cmp(&b.executable_path))
        });
    }
    apps
}
