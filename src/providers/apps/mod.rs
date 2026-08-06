//! App search provider — Start Menu index, fuzzy match, frecency, icons.
//!
//! The 975-line monolith was split by concern:
//! - [`scan`] — Start Menu walking, `.lnk` resolution, dedup
//! - [`fuzzy`] — character-level fuzzy scorer
//! - [`icons`] — `.ico`/`IShellItemImageFactory` extraction + PNG cache

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider, SearchResult};

mod fuzzy;
mod icons;
mod scan;

#[derive(Clone)]
struct InstalledApp {
    name: String,
    executable_path: String,
    icon_rgba: Option<(Vec<u8>, u32, u32)>,
}

pub struct AppsProvider {
    apps: Arc<Mutex<Vec<InstalledApp>>>,
    search_dirs: Vec<String>,
    refresh_in_progress: Arc<AtomicBool>,
    revision: Arc<AtomicU64>,
}

impl AppsProvider {
    pub fn new(search_dirs: Vec<String>) -> Self {
        let provider = Self {
            apps: Arc::new(Mutex::new(Vec::new())),
            search_dirs,
            refresh_in_progress: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        };
        provider.refresh();
        provider
    }

    fn result_for(app: &InstalledApp, score: f64) -> SearchResult {
        SearchResult {
            title: app.name.clone(),
            subtitle: "Application".into(),
            kind: "app".into(),
            provider_id: "apps".into(),
            action: app.executable_path.clone(),
            icon_rgba: app.icon_rgba.clone(),
            score,
        }
    }

    fn recommendations(&self, ctx: &SearchContext, limit: usize) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = Vec::new();
        let frecency = ctx.db.top_frecency(limit);
        let apps = match self.apps.lock() {
            Ok(a) => a,
            Err(_) => return results,
        };

        for (app_key, _count, _last) in &frecency {
            if let Some(app) = apps
                .iter()
                .find(|app| app.executable_path == *app_key || app.name == *app_key)
            {
                // Legacy title keys and current executable-path keys can both exist
                // for one launch. Keep one recommendation for the real action.
                if !results
                    .iter()
                    .any(|result| result.action == app.executable_path)
                {
                    results.push(Self::result_for(app, 200.0));
                }
            }
        }

        for app in apps.iter() {
            if results.len() >= limit {
                break;
            }
            if !results
                .iter()
                .any(|result| result.action == app.executable_path)
            {
                results.push(Self::result_for(app, 100.0));
            }
        }

        results
    }
}

impl SearchProvider for AppsProvider {
    fn id(&self) -> &'static str {
        "apps"
    }

    fn priority(&self) -> i32 {
        0
    }

    fn should_run(&self, _query: &str) -> bool {
        true
    }

    fn search(&self, ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let q = query.trim().to_lowercase();

        if q.is_empty() {
            return self.recommendations(ctx, 12);
        }

        let apps = match self.apps.lock() {
            Ok(a) => a,
            Err(_) => return Vec::new(),
        };

        let mut results: Vec<SearchResult> = Vec::new();
        for app in apps.iter() {
            if let Some(score) = fuzzy::fuzzy_score(&q, &app.name) {
                let frecency = ctx
                    .db
                    .frecency_score(&app.executable_path)
                    .max(ctx.db.frecency_score(&app.name));
                let boost = 1.0 + (frecency * 5.0).min(2.0);
                results.push(Self::result_for(app, score * boost));
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.title.cmp(&b.title))
        });

        results
    }

    fn activate(&self, ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        let executable = Path::new(&result.action);
        if !is_executable_path(executable) || !executable.is_file() {
            return Err(ElementError::Other(format!(
                "application executable is unavailable: {}",
                result.action
            )));
        }

        let mut command = std::process::Command::new(executable);
        if let Some(working_dir) = executable.parent() {
            command.current_dir(working_dir);
        }
        command.spawn().map_err(ElementError::Io)?;
        ctx.db.record_launch(&result.action);
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

        let apps = Arc::clone(&self.apps);
        let search_dirs = self.search_dirs.clone();
        let refresh_in_progress = Arc::clone(&self.refresh_in_progress);
        let revision = Arc::clone(&self.revision);
        let worker = std::thread::Builder::new()
            .name("element-app-index".into())
            .spawn(move || {
                let indexed_apps =
                    std::panic::catch_unwind(|| scan::scan_apps(&search_dirs)).unwrap_or_default();
                if let Ok(mut guard) = apps.lock() {
                    *guard = indexed_apps;
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

/// True when `path` points at an `.exe` (case-insensitive).
pub(crate) fn is_executable_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::database::Database;

    #[test]
    fn only_executable_targets_are_indexed() {
        assert!(is_executable_path(std::path::Path::new(
            "C:\\Apps\\Element.EXE"
        )));
        assert!(!is_executable_path(std::path::Path::new(
            "C:\\Apps\\Element.lnk"
        )));
    }

    #[test]
    fn recommendations_dedupe_legacy_and_executable_frecency_keys() {
        let executable_path = r"C:\\Apps\\Example.exe".to_string();
        let provider = AppsProvider {
            apps: Arc::new(Mutex::new(vec![InstalledApp {
                name: "Example".into(),
                executable_path: executable_path.clone(),
                icon_rgba: None,
            }])),
            search_dirs: Vec::new(),
            refresh_in_progress: Arc::new(AtomicBool::new(false)),
            revision: Arc::new(AtomicU64::new(0)),
        };
        let db = Database::new_in_memory();
        db.record_launch("Example");
        db.record_launch(&executable_path);
        let config = Config::default();
        let context = SearchContext {
            config: &config,
            db: &db,
        };

        let results = provider.recommendations(&context, 12);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].action, executable_path);
    }
}
