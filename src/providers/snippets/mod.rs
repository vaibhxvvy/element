//! Snippets provider — quick text insertions from `~/.element/snippets.toml`.
//!
//! The file is a TOML table of name → text:
//!
//! ```toml
//! [snippets]
//! email = "vaibh@example.com"
//! ```
//!
//! Type a snippet name and Enter copies its text; `snip` / `snippet` lists
//! everything (optionally filtered: `snip invoice`). Snippets are cached in
//! memory and reloaded on every refresh (i.e. each time the overlay opens),
//! so edits apply without restarting Element.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider, SearchResult};

const LIST_PREFIXES: [&str; 2] = ["snip", "snippet"];
const LIST_SCORE: f64 = 500.0;
const NAME_SCORE: f64 = 700.0;
const MAX_LISTED: usize = 15;

#[derive(Debug, Default, serde::Deserialize)]
struct SnippetsFile {
    snippets: HashMap<String, String>,
}

/// Read `snippets.toml` (name → text), sorted by name.
fn load_snippets(path: &Path) -> Vec<(String, String)> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(parsed): Result<SnippetsFile, _> = toml::from_str(&raw) else {
        return Vec::new();
    };
    let mut items: Vec<(String, String)> = parsed.snippets.into_iter().collect();
    items.sort_by(|a, b| a.0.cmp(&b.0));
    items
}

fn snippets_path() -> std::path::PathBuf {
    crate::config::data_dir().join("snippets.toml")
}

/// Create the snippets file with a commented example on first run so the
/// feature is discoverable. Never overwrites an existing file.
fn ensure_snippets_file(path: &Path) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        path,
        "# Element snippets — name = \"text\".\n\
         # Type a name and press Enter to copy its text to the clipboard.\n\
         # Type \"snip\" to list everything.\n\
         [snippets]\n\
         # email = \"you@example.com\"\n",
    );
}

pub struct SnippetsProvider {
    snippets: Mutex<Vec<(String, String)>>,
}

impl SnippetsProvider {
    pub fn new() -> Self {
        let path = snippets_path();
        ensure_snippets_file(&path);
        Self::from_path(&path)
    }

    /// Test helper: build a provider seeded from an explicit file.
    fn from_path(path: &Path) -> Self {
        Self {
            snippets: Mutex::new(load_snippets(path)),
        }
    }

    fn snapshot(&self) -> Vec<(String, String)> {
        self.snippets
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn is_listing(q: &str) -> bool {
        LIST_PREFIXES
            .iter()
            .any(|p| q == *p || q.starts_with(&format!("{p} ")))
    }
}

impl SearchProvider for SnippetsProvider {
    fn id(&self) -> &'static str {
        "snippets"
    }

    fn priority(&self) -> i32 {
        8
    }

    fn should_run(&self, query: &str) -> bool {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return false;
        }
        Self::is_listing(&q)
            || self
                .snapshot()
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(&q))
    }

    fn search(&self, _ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let q = query.trim().to_ascii_lowercase();
        let listing = Self::is_listing(&q);
        let mut items = self.snapshot();

        if listing {
            let term = q.split_once(' ').map(|(_, rest)| rest.trim()).unwrap_or("");
            if !term.is_empty() {
                items.retain(|(name, text)| {
                    name.to_ascii_lowercase().contains(term)
                        || text.to_ascii_lowercase().contains(term)
                });
            }
            items.truncate(MAX_LISTED);
        } else {
            items.retain(|(name, _)| name.eq_ignore_ascii_case(&q));
            if items.is_empty() {
                return Vec::new();
            }
        }

        items
            .into_iter()
            .enumerate()
            .map(|(i, (name, text))| {
                let trimmed: String = text.chars().take(48).collect();
                let subtitle = if text.chars().count() > 48 {
                    format!("{trimmed}…")
                } else {
                    trimmed
                };
                SearchResult {
                    title: name.clone(),
                    subtitle,
                    kind: "snippet".into(),
                    provider_id: "snippets".into(),
                    action: text,
                    icon_rgba: None,
                    score: if listing {
                        LIST_SCORE - i as f64
                    } else {
                        NAME_SCORE
                    },
                }
            })
            .collect()
    }

    fn refresh(&self) {
        *self.snippets.lock().unwrap_or_else(|p| p.into_inner()) = load_snippets(&snippets_path());
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| ElementError::Other(format!("clipboard error: {e:?}")))?;
        clipboard
            .set_text(result.action.clone())
            .map_err(|e| ElementError::Other(format!("clipboard error: {e:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::database::Database;
    use std::io::Write;

    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::OnceLock;

    fn unique_snippets_path() -> std::path::PathBuf {
        static COUNTER: OnceLock<AtomicU32> = OnceLock::new();
        let n = COUNTER
            .get_or_init(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "element-snippets-test-{}-{n}.toml",
            std::process::id()
        ))
    }

    fn temp_snippets(content: &str) -> std::path::PathBuf {
        let path = unique_snippets_path();
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    fn provider_with(content: &str) -> SnippetsProvider {
        let path = temp_snippets(content);
        let provider = SnippetsProvider::from_path(&path);
        std::fs::remove_file(&path).ok();
        provider
    }

    fn ctx<'a>(config: &'a Config, db: &'a Database) -> SearchContext<'a> {
        SearchContext { config, db }
    }

    #[test]
    fn loads_names_and_text_sorted() {
        let path =
            temp_snippets("[snippets]\nzeta = \"z\"\nalpha = \"hello world\"\nemail = \"a@b.c\"\n");
        let items = load_snippets(&path);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].0, "alpha");
        assert_eq!(items[0].1, "hello world");
        assert_eq!(items[2].0, "zeta");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn missing_file_is_empty() {
        let path = std::env::temp_dir().join("element-snippets-test-does-not-exist.toml");
        assert!(load_snippets(&path).is_empty());
    }

    #[test]
    fn listing_filters_by_term() {
        let provider =
            provider_with("[snippets]\nemail = \"you@example.com\"\nphone = \"+1 555\"\n");
        let config = Config::default();
        let db = Database::new_in_memory();
        let context = ctx(&config, &db);
        // snip lists everything, sorted
        let results = provider.search(&context, "snip");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "email");
        assert_eq!(results[0].kind, "snippet");
        assert_eq!(results[0].score, 500.0);
        // snip with a term filters on name or text
        let results = provider.search(&context, "snip example");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "email");
    }

    #[test]
    fn exact_name_match_scores_high() {
        let provider = provider_with("[snippets]\nemail = \"you@example.com\"\n");
        let config = Config::default();
        let db = Database::new_in_memory();
        let context = ctx(&config, &db);
        let results = provider.search(&context, "email");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, "you@example.com");
        assert_eq!(results[0].score, 700.0);
        assert!(provider.should_run("email"));
        assert!(!provider.should_run("ema"));
    }
}
