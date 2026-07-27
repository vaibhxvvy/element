use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::database::Database;

#[derive(Clone)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub kind: String,
}

#[derive(Clone)]
pub struct InstalledApp {
    pub name: String,
    pub path: String,
}

#[derive(Clone)]
pub struct SearchEngine {
    config: Arc<Config>,
    db: Arc<Database>,
    apps: Arc<Mutex<Vec<InstalledApp>>>,
}

impl SearchEngine {
    pub fn new(config: &Config, db: Arc<Database>) -> Self {
        let engine = Self {
            config: Arc::new(config.clone()),
            db,
            apps: Arc::new(Mutex::new(Vec::new())),
        };
        engine.refresh_apps();
        engine
    }

    pub fn refresh_apps(&self) {
        let mut apps = Vec::new();
        #[cfg(target_os = "windows")]
        {
            let start_menu_dirs = vec![
                std::env::var("ProgramData").map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
                std::env::var("APPDATA").map(|p| format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", p)),
            ];

            for dir in start_menu_dirs.into_iter().filter_map(|d| d.ok()) {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map(|e| e == "lnk").unwrap_or(false) {
                            let name = path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();
                            if !name.is_empty() && !name.starts_with('.') {
                                apps.push(InstalledApp { name, path: path.to_string_lossy().to_string() });
                            }
                        }
                    }
                }
            }
            apps.sort_by(|a, b| a.name.cmp(&b.name));
        }
        *self.apps.lock().unwrap() = apps;
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        if query.is_empty() {
            return vec![
                SearchResult { title: "Help".into(), subtitle: "Type to search anything — apps, files, calc, emoji".into(), kind: "help".into() },
            ];
        }

        let q = query.trim().to_lowercase();
        let mut results = Vec::new();

        if q.chars().any(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '*' | '/' | 'x' | '÷' | '(' | ')')) {
            let expr = q.replace('x', "*").replace('÷', "/");
            if let Ok(val) = evalexpr::eval(&expr) {
                results.push(SearchResult {
                    title: format!("= {}", val),
                    subtitle: format!("Calc: {}", query),
                    kind: "calc".into(),
                });
            }
        }

        if q.starts_with("emoji") || q.starts_with(":") {
            let term = q.trim_start_matches("emoji").trim().trim_start_matches(':').trim();
            for emoji in emojis::iter() {
                let name = emoji.name().to_lowercase();
                let codes: Vec<String> = emoji.shortcodes().map(|s| s.to_string()).collect();
                if term.is_empty() || name.contains(term) || codes.iter().any(|c| c.contains(term)) {
                    results.push(SearchResult {
                        title: format!("{}  {}", emoji.as_str(), codes.first().map(|c| format!(":{}:", c)).unwrap_or_default()),
                        subtitle: emoji.name().into(),
                        kind: "emoji".into(),
                    });
                    if results.len() > 20 { break; }
                }
            }
        }

        if q == "cbhist" || q.starts_with("clip") {
            let entries = self.db.load_clipboard(20);
            for (text, ts) in &entries {
                let preview: String = text.lines().next().unwrap_or(text).chars().take(80).collect();
                results.push(SearchResult {
                    title: preview,
                    subtitle: format!("Clipboard · {}", ts),
                    kind: "clipboard".into(),
                });
            }
        }

        let apps = self.apps.lock().unwrap();
        for app in apps.iter() {
            if app.name.to_lowercase().contains(&q) {
                results.push(SearchResult {
                    title: app.name.clone(),
                    subtitle: "App".into(),
                    kind: "app".into(),
                });
            }
        }
        drop(apps);

        results.push(SearchResult {
            title: format!("Search web for \"{}\"", query),
            subtitle: config_or_default(&self.config.search_url, "https://duckduckgo.com/search?q=%s").replace("%s", query),
            kind: "websearch".into(),
        });

        results
    }

    pub fn activate(&self, kind: &str, title: &str, input: &str) {
        match kind {
            "app" => {
                let apps = self.apps.lock().unwrap();
                if let Some(app) = apps.iter().find(|a| a.name == title) {
                    let _ = std::process::Command::new("cmd")
                        .args(["/c", "start", "", &app.path])
                        .spawn();
                }
            }
            "websearch" => {
                let _ = webbrowser::open(&self.config.search_url.replace("%s", input));
            }
            "calc" => {
                let _ = arboard::Clipboard::new()
                    .and_then(|mut c| c.set_text(title.trim_start_matches("= ")));
            }
            "emoji" => {
                if let Some(emoji_char) = title.chars().next() {
                    let _ = arboard::Clipboard::new()
                        .and_then(|mut c| c.set_text(emoji_char.to_string()));
                }
            }
            "clipboard" => {
                let _ = arboard::Clipboard::new()
                    .and_then(|mut c| c.set_text(title));
            }
            _ => {}
        }
    }
}

fn config_or_default(config: &str, default: &str) -> String {
    if config.is_empty() { default.into() } else { config.into() }
}
