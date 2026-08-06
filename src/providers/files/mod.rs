//! File & folder search provider — Raycast-style "file search" mode.
//!
//! Prefix-gated so it never shadows normal app search:
//! - `file <query>`  — search files and folders
//! - `folder <query>` — folders only
//!
//! A background thread indexes the configured directories (default: the user
//! home folder) with sensible exclusions and caps. Results are fuzzy-matched
//! by name; icons are extracted lazily on a worker thread and published
//! through the provider revision, so the UI re-renders them without ever
//! blocking the UI thread on COM.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider, SearchResult};

use super::fuzzy::fuzzy_score;
use super::icon::{cached_icon_for_path, icon_cache_dir};

mod scan;

/// Maximum results returned per query.
const MAX_RESULTS: usize = 30;
/// How many results look ahead for icon extraction.
const ICON_LOOKAHEAD: usize = 12;

#[derive(Clone)]
pub(crate) struct FileEntry {
    pub(crate) name: String,
    pub(crate) path: String,
    pub(crate) is_dir: bool,
}

/// In-memory icon cache: path → RGBA (or `None` when extraction failed).
type IconCache = Arc<Mutex<HashMap<String, Option<(Vec<u8>, u32, u32)>>>>;

/// What kind of entries a `file`/`folder` prefix query should return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    Any,
    FoldersOnly,
}

/// Parse a Raycast-style prefix query into a mode + search term.
///
/// `"file notes"` → `(Any, "notes")`; `"folders"` → `(FoldersOnly, "")`;
/// `"firefox"` → `None` (not a file-search query).
fn parse_prefix(query: &str) -> Option<(SearchMode, String)> {
    let q = query.trim().to_lowercase();
    let (mode, prefix) = if q == "file" || q.starts_with("file ") {
        (SearchMode::Any, "file")
    } else if q == "files" || q.starts_with("files ") {
        (SearchMode::Any, "files")
    } else if q == "folder" || q.starts_with("folder ") {
        (SearchMode::FoldersOnly, "folder")
    } else if q == "folders" || q.starts_with("folders ") {
        (SearchMode::FoldersOnly, "folders")
    } else {
        return None;
    };
    let term = q[prefix.len()..].trim().to_string();
    Some((mode, term))
}

pub struct FilesProvider {
    entries: Arc<Mutex<Vec<FileEntry>>>,
    /// Path → cached icon (or `None` when extraction failed).
    icons: IconCache,
    search_dirs: Vec<String>,
    roots: Arc<Mutex<Vec<String>>>,
    refresh_in_progress: Arc<AtomicBool>,
    icon_worker_busy: Arc<AtomicBool>,
    revision: Arc<AtomicU64>,
}

impl FilesProvider {
    pub fn new(search_dirs: Vec<String>) -> Self {
        let provider = Self {
            entries: Arc::new(Mutex::new(Vec::new())),
            icons: Arc::new(Mutex::new(HashMap::new())),
            search_dirs,
            roots: Arc::new(Mutex::new(Vec::new())),
            refresh_in_progress: Arc::new(AtomicBool::new(false)),
            icon_worker_busy: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        };
        provider.refresh();
        provider
    }

    fn result_for(entry: &FileEntry, score: f64) -> SearchResult {
        let parent = Path::new(&entry.path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        SearchResult {
            title: entry.name.clone(),
            subtitle: parent,
            kind: if entry.is_dir { "folder" } else { "file" }.into(),
            provider_id: "files".into(),
            action: entry.path.clone(),
            icon_rgba: None,
            score,
        }
    }

    /// Fill `icon_rgba` from the in-memory cache for each result.
    fn attach_icons(&self, results: &mut [SearchResult]) {
        let Ok(icons) = self.icons.lock() else {
            return;
        };
        for result in results.iter_mut() {
            if let Some(icon) = icons.get(&result.action).cloned() {
                result.icon_rgba = icon;
            }
        }
    }

    /// Enqueue lazy icon extraction for the top results that have no icon yet.
    /// Runs on a worker thread; bumps `revision` when it publishes new icons
    /// so the UI re-runs the query and renders them.
    fn fetch_icons_async(&self, results: &[SearchResult]) {
        if self
            .icon_worker_busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let mut missing: Vec<String> = Vec::new();
        {
            let Ok(icons) = self.icons.lock() else {
                self.icon_worker_busy.store(false, Ordering::SeqCst);
                return;
            };
            for result in results.iter().take(ICON_LOOKAHEAD) {
                if result.icon_rgba.is_none() && !icons.contains_key(&result.action) {
                    missing.push(result.action.clone());
                }
            }
        }
        if missing.is_empty() {
            self.icon_worker_busy.store(false, Ordering::SeqCst);
            return;
        }

        let icons = Arc::clone(&self.icons);
        let busy = Arc::clone(&self.icon_worker_busy);
        let revision = Arc::clone(&self.revision);
        let worker = std::thread::Builder::new()
            .name("element-file-icons".into())
            .spawn(move || {
                let cache_dir = icon_cache_dir();
                let mut added = false;
                for path in &missing {
                    let icon = cached_icon_for_path(Path::new(path), &cache_dir);
                    added |= icon.is_some();
                    if let Ok(mut icons) = icons.lock() {
                        icons.insert(path.clone(), icon);
                    }
                }
                if added {
                    revision.fetch_add(1, Ordering::SeqCst);
                }
                busy.store(false, Ordering::SeqCst);
            });

        if worker.is_err() {
            self.icon_worker_busy.store(false, Ordering::SeqCst);
        }
    }

    fn recommendations(&self, mode: SearchMode) -> Vec<SearchResult> {
        // Empty query: surface the scan roots (Desktop, Documents, ...) so the
        // user always sees meaningful folders, never arbitrary junk.
        let roots = match self.roots.lock() {
            Ok(roots) => roots.clone(),
            Err(_) => return Vec::new(),
        };
        let mut results: Vec<SearchResult> = Vec::new();
        for root in roots {
            if mode == SearchMode::FoldersOnly {
                // All roots are folders; nothing to filter out.
            }
            let path = Path::new(&root);
            let name = path
                .file_name()
                .or_else(|| path.parent().and_then(|p| p.file_name()))
                .and_then(|s| s.to_str())
                .unwrap_or(&root)
                .to_string();
            if name.is_empty() {
                continue;
            }
            results.push(SearchResult {
                title: name,
                subtitle: root.clone(),
                kind: "folder".into(),
                provider_id: "files".into(),
                action: root.clone(),
                icon_rgba: None,
                score: 210.0,
            });
        }
        results
    }
}

impl SearchProvider for FilesProvider {
    fn id(&self) -> &'static str {
        "files"
    }

    fn priority(&self) -> i32 {
        5
    }

    fn should_run(&self, query: &str) -> bool {
        parse_prefix(query).is_some()
    }

    fn search(&self, _ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let Some((mode, term)) = parse_prefix(query) else {
            return Vec::new();
        };

        let mut results = if term.is_empty() {
            self.recommendations(mode)
        } else {
            let Ok(entries) = self.entries.lock() else {
                return Vec::new();
            };
            let mut results: Vec<SearchResult> = Vec::new();
            for entry in entries.iter() {
                if mode == SearchMode::FoldersOnly && !entry.is_dir {
                    continue;
                }
                if let Some(score) = fuzzy_score(&term, &entry.name) {
                    let score = 120.0 + score * 2.0 + if entry.is_dir { 10.0 } else { 0.0 };
                    results.push(Self::result_for(entry, score));
                }
            }
            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.title.cmp(&b.title))
            });
            results.truncate(MAX_RESULTS);
            results
        };

        self.attach_icons(&mut results);
        self.fetch_icons_async(&results);
        results
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        let path = Path::new(&result.action);
        if !path.exists() {
            return Err(ElementError::Other(format!(
                "path no longer exists: {}",
                result.action
            )));
        }
        // explorer.exe opens folders in Explorer and files with their default
        // handler — the same behavior as a double-click, no console flash.
        std::process::Command::new("explorer.exe")
            .arg(&result.action)
            .spawn()
            .map_err(ElementError::Io)?;
        Ok(())
    }

    fn refresh(&self) {
        if self
            .refresh_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let entries = Arc::clone(&self.entries);
        let roots = Arc::clone(&self.roots);
        let search_dirs = self.search_dirs.clone();
        let refresh_in_progress = Arc::clone(&self.refresh_in_progress);
        let revision = Arc::clone(&self.revision);
        let worker = std::thread::Builder::new()
            .name("element-file-index".into())
            .spawn(move || {
                let resolved_roots = scan::resolve_roots(&search_dirs);
                let indexed =
                    std::panic::catch_unwind(|| scan::scan_files(&search_dirs)).unwrap_or_default();
                let mut replaced = false;
                if let Ok(mut guard) = entries.lock() {
                    *guard = indexed;
                    replaced = true;
                }
                if let Ok(mut guard) = roots.lock() {
                    *guard = resolved_roots;
                    replaced = true;
                }
                if replaced {
                    revision.fetch_add(1, Ordering::SeqCst);
                }
                refresh_in_progress.store(false, Ordering::SeqCst);
            });

        if worker.is_err() {
            self.refresh_in_progress.store(false, Ordering::SeqCst);
        }
    }

    fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_parses_file_mode() {
        assert_eq!(
            parse_prefix("file notes"),
            Some((SearchMode::Any, "notes".to_string()))
        );
        assert_eq!(
            parse_prefix("file"),
            Some((SearchMode::Any, "".to_string()))
        );
        assert_eq!(
            parse_prefix("files notes"),
            Some((SearchMode::Any, "notes".to_string()))
        );
    }

    #[test]
    fn prefix_parses_folder_mode() {
        assert_eq!(
            parse_prefix("folder documents"),
            Some((SearchMode::FoldersOnly, "documents".to_string()))
        );
        assert_eq!(
            parse_prefix("folders"),
            Some((SearchMode::FoldersOnly, "".to_string()))
        );
    }

    #[test]
    fn prefix_case_insensitive_and_trimmed() {
        assert_eq!(
            parse_prefix("  FILE Report "),
            Some((SearchMode::Any, "report".to_string()))
        );
    }

    #[test]
    fn non_prefix_queries_do_not_trigger() {
        assert_eq!(parse_prefix("firefox"), None);
        assert_eq!(parse_prefix("f"), None);
        assert_eq!(parse_prefix("filex"), None);
        assert_eq!(parse_prefix(""), None);
    }

    #[test]
    fn fuzzy_search_ranks_folders_over_files() {
        let provider = FilesProvider {
            entries: Arc::new(Mutex::new(vec![
                FileEntry {
                    name: "Reports".into(),
                    path: r"C:\Users\me\Documents\Reports".into(),
                    is_dir: true,
                },
                FileEntry {
                    name: "report_q3.txt".into(),
                    path: r"C:\Users\me\Documents\report_q3.txt".into(),
                    is_dir: false,
                },
            ])),
            icons: Arc::new(Mutex::new(HashMap::new())),
            search_dirs: Vec::new(),
            roots: Arc::new(Mutex::new(Vec::new())),
            refresh_in_progress: Arc::new(AtomicBool::new(false)),
            icon_worker_busy: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        };
        let config = crate::config::Config::default();
        let db = crate::database::Database::new_in_memory();
        let ctx = SearchContext {
            config: &config,
            db: &db,
        };

        let results = provider.search(&ctx, "file report");
        assert!(!results.is_empty());
        assert_eq!(results[0].kind, "folder");
        assert_eq!(results[0].title, "Reports");
        assert_eq!(results[0].provider_id, "files");
    }

    #[test]
    fn folder_mode_excludes_files() {
        let provider = FilesProvider {
            entries: Arc::new(Mutex::new(vec![
                FileEntry {
                    name: "Notes".into(),
                    path: r"C:\Users\me\Documents\Notes".into(),
                    is_dir: true,
                },
                FileEntry {
                    name: "notes.txt".into(),
                    path: r"C:\Users\me\Documents\notes.txt".into(),
                    is_dir: false,
                },
            ])),
            icons: Arc::new(Mutex::new(HashMap::new())),
            search_dirs: Vec::new(),
            roots: Arc::new(Mutex::new(Vec::new())),
            refresh_in_progress: Arc::new(AtomicBool::new(false)),
            icon_worker_busy: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        };
        let config = crate::config::Config::default();
        let db = crate::database::Database::new_in_memory();
        let ctx = SearchContext {
            config: &config,
            db: &db,
        };

        let results = provider.search(&ctx, "folder notes");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "folder");
    }

    #[test]
    fn empty_query_recommends_roots() {
        let provider = FilesProvider {
            entries: Arc::new(Mutex::new(Vec::new())),
            icons: Arc::new(Mutex::new(HashMap::new())),
            search_dirs: Vec::new(),
            roots: Arc::new(Mutex::new(vec![
                r"C:\Users\me\Desktop".into(),
                r"C:\Users\me\Documents".into(),
            ])),
            refresh_in_progress: Arc::new(AtomicBool::new(false)),
            icon_worker_busy: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        };
        let config = crate::config::Config::default();
        let db = crate::database::Database::new_in_memory();
        let ctx = SearchContext {
            config: &config,
            db: &db,
        };

        let results = provider.search(&ctx, "file");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Desktop");
        assert_eq!(results[0].kind, "folder");
        assert_eq!(results[0].action, r"C:\Users\me\Desktop");
        assert!(results.iter().all(|r| r.kind == "folder"));
    }
}
