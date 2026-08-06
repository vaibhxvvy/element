//! File & folder search provider — Raycast-style file search.
//!
//! - `file <query>`  — search files and folders (explicit mode)
//! - `folder <query>` — folders only
//! - any other query — also fuzzy-matches file names (bare mode), scored
//!   lower and capped tighter so app search stays dominant; queries owned by
//!   other providers (emoji `:…`, clipboard `cbhist`/`clip`, math) are skipped.
//!
//! A background thread indexes the configured directories (default: curated
//! user folders) with sensible exclusions and caps. Results are fuzzy-matched
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

/// Maximum results returned per prefixed query.
const MAX_RESULTS: usize = 30;
/// Maximum results returned for a bare (non-prefixed) query — keep file
/// matches from flooding normal app search.
const BARE_RESULTS: usize = 6;
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

/// A parsed file-search request.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedQuery {
    mode: SearchMode,
    term: String,
    /// True for bare queries (no `file`/`folder` prefix). Bare matches are
    /// scored lower and capped tighter so normal app search stays dominant.
    bare: bool,
}

/// Parse any query into a file search. Explicit `file`/`folder` prefixes set
/// the mode; every other query (≥ 2 chars and not another provider's domain)
/// also searches files — Raycast-style bare file search.
fn parse_query(query: &str) -> Option<ParsedQuery> {
    if let Some((mode, term)) = parse_prefix(query) {
        return Some(ParsedQuery {
            mode,
            term,
            bare: false,
        });
    }
    let term = query.trim().to_lowercase();
    if term.len() < 2 || is_other_domain(&term) {
        return None;
    }
    Some(ParsedQuery {
        mode: SearchMode::Any,
        term,
        bare: true,
    })
}

/// Queries owned by other providers shouldn't also fuzzy-match file names:
/// emoji (`:…`, `emoji`), clipboard history (`cbhist`, `clip`) and math
/// expressions (digits + an operator).
fn is_other_domain(query: &str) -> bool {
    query.starts_with(':')
        || query.starts_with("emoji")
        || query.starts_with("cbhist")
        || query.starts_with("clip")
        || (query.chars().any(|c| "+-*/%^=".contains(c))
            && query.chars().any(|c| c.is_ascii_digit()))
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
        // Publish the scan roots immediately (cheap is_dir checks) so bare
        // `file`/`folder` queries show Desktop/Documents/... right away,
        // before the background walk completes.
        let initial_roots = scan::resolve_roots(&provider.search_dirs);
        if let Ok(mut guard) = provider.roots.lock() {
            *guard = initial_roots;
        }
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
                if result.icon_rgba.is_none()
                    && !result.action.is_empty()
                    && !icons.contains_key(&result.action)
                {
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

    /// True while the very first background index is still running and no
    /// entries have been published yet — the window where `file <query>`
    /// would otherwise show nothing.
    fn first_scan_in_progress(&self) -> bool {
        self.refresh_in_progress.load(Ordering::SeqCst)
            && self
                .entries
                .lock()
                .map(|entries| entries.is_empty())
                .unwrap_or(false)
    }

    /// Placeholder result shown while the first scan runs, so `file report`
    /// never silently returns an empty list right after launch.
    fn indexing_hint() -> SearchResult {
        SearchResult {
            title: "Indexing your files…".into(),
            subtitle: "Results appear here once the scan finishes (a few seconds)".into(),
            kind: "hint".into(),
            provider_id: "files".into(),
            action: String::new(),
            icon_rgba: None,
            score: 0.0,
        }
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
        parse_query(query).is_some()
    }

    fn search(&self, _ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let Some(pq) = parse_query(query) else {
            return Vec::new();
        };

        let mut results = if pq.term.is_empty() {
            self.recommendations(pq.mode)
        } else if self.first_scan_in_progress() {
            // No index yet — show a placeholder instead of a silent empty list.
            vec![Self::indexing_hint()]
        } else {
            let Ok(entries) = self.entries.lock() else {
                return Vec::new();
            };
            // Bare queries score lower and cap tighter so app results stay
            // on top; explicit `file`/`folder` queries rank higher.
            let (base, per_char, folder_bonus, cap) = if pq.bare {
                (10.0, 0.5, 5.0, BARE_RESULTS)
            } else {
                (120.0, 2.0, 10.0, MAX_RESULTS)
            };
            let mut results: Vec<SearchResult> = Vec::new();
            for entry in entries.iter() {
                if pq.mode == SearchMode::FoldersOnly && !entry.is_dir {
                    continue;
                }
                if let Some(score) = fuzzy_score(&pq.term, &entry.name) {
                    let score =
                        base + score * per_char + if entry.is_dir { folder_bonus } else { 0.0 };
                    results.push(Self::result_for(entry, score));
                }
            }
            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.title.cmp(&b.title))
            });
            results.truncate(cap);
            results
        };

        self.attach_icons(&mut results);
        self.fetch_icons_async(&results);
        results
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        if result.kind == "hint" {
            return Ok(());
        }
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
    fn bare_queries_search_files_too() {
        let q = parse_query("firefox").unwrap();
        assert!(q.bare);
        assert_eq!(q.mode, SearchMode::Any);
        assert_eq!(q.term, "firefox");

        let q = parse_query(".png").unwrap();
        assert!(q.bare);
        assert_eq!(q.term, ".png");

        let q = parse_query("pvt").unwrap();
        assert!(q.bare);
        assert_eq!(q.term, "pvt");
    }

    #[test]
    fn prefixed_queries_are_not_bare() {
        let q = parse_query("file report").unwrap();
        assert!(!q.bare);
        assert_eq!(q.mode, SearchMode::Any);
        assert_eq!(q.term, "report");

        let q = parse_query("folder docs").unwrap();
        assert!(!q.bare);
        assert_eq!(q.mode, SearchMode::FoldersOnly);
        assert_eq!(q.term, "docs");
    }

    #[test]
    fn bare_queries_skip_short_and_other_domains() {
        assert!(parse_query("f").is_none());
        assert!(parse_query("").is_none());
        assert!(parse_query(":smile").is_none());
        assert!(parse_query("emoji").is_none());
        assert!(parse_query("cbhist").is_none());
        assert!(parse_query("clip").is_none());
        assert!(parse_query("2+2").is_none());
        // Spelled-out math still searches files.
        assert!(parse_query("two plus two").is_some());
    }

    #[test]
    fn bare_search_scores_low_and_caps_tight() {
        let mut entries = Vec::new();
        for i in 0..10 {
            entries.push(FileEntry {
                name: format!("report_{i}.txt"),
                path: format!(r"C:\Users\me\Documents\report_{i}.txt"),
                is_dir: false,
            });
        }
        let provider = FilesProvider {
            entries: Arc::new(Mutex::new(entries)),
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

        // Bare query: capped at 6, all below the prefixed score floor (120).
        let results = provider.search(&ctx, "report");
        assert_eq!(results.len(), BARE_RESULTS);
        assert!(results.iter().all(|r| r.provider_id == "files"));
        assert!(results.iter().all(|r| r.score < 120.0));

        // Same query with the explicit prefix: 10 results, high scoring.
        let results = provider.search(&ctx, "file report");
        assert_eq!(results.len(), 10);
        assert!(results.iter().all(|r| r.score >= 120.0));
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

    #[test]
    fn first_scan_shows_indexing_hint_instead_of_empty_list() {
        let provider = FilesProvider {
            entries: Arc::new(Mutex::new(Vec::new())),
            icons: Arc::new(Mutex::new(HashMap::new())),
            search_dirs: Vec::new(),
            roots: Arc::new(Mutex::new(Vec::new())),
            refresh_in_progress: Arc::new(AtomicBool::new(true)),
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
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "hint");
        assert!(results[0].provider_id == "files");
        // Activating the hint is a no-op, never an error.
        assert!(provider.activate(&ctx, &results[0]).is_ok());
    }

    #[test]
    fn no_hint_once_index_has_entries() {
        let provider = FilesProvider {
            entries: Arc::new(Mutex::new(vec![FileEntry {
                name: "report_q3.txt".into(),
                path: r"C:\Users\me\Documents\report_q3.txt".into(),
                is_dir: false,
            }])),
            icons: Arc::new(Mutex::new(HashMap::new())),
            search_dirs: Vec::new(),
            roots: Arc::new(Mutex::new(Vec::new())),
            refresh_in_progress: Arc::new(AtomicBool::new(true)),
            icon_worker_busy: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        };
        let config = crate::config::Config::default();
        let db = crate::database::Database::new_in_memory();
        let ctx = SearchContext {
            config: &config,
            db: &db,
        };

        // No match exists, but the index is populated — real results only.
        let results = provider.search(&ctx, "file zzzz");
        assert!(results.is_empty());
    }
}
