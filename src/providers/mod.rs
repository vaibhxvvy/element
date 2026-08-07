use crate::config::Config;
use crate::database::Database;
use crate::error::ElementError;

pub mod apps;
pub mod calculator;
pub mod clipboard;
pub mod emoji;
pub mod files;
pub mod help;
pub mod settings;
pub mod system;
pub mod units;
pub mod websearch;

/// Shared character-level fuzzy scorer (used by apps and files).
mod fuzzy;
/// Shared icon pipeline: on-disk PNG cache + `IShellItemImageFactory`
/// extraction for any shell path (executables, files, folders).
pub mod icon;

/// One actionable search result, owned by the provider that produced it.
///
/// `action` carries the exact provider-owned data needed to activate this
/// exact result — never recover a selected item by its visible title, because
/// titles are not unique.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub subtitle: String,
    pub kind: String,
    pub provider_id: String,
    /// Provider-owned data used to activate this exact result.
    pub action: String,
    pub icon_rgba: Option<(Vec<u8>, u32, u32)>,
    pub score: f64,
}

pub struct SearchContext<'a> {
    pub config: &'a Config,
    pub db: &'a Database,
}

pub trait SearchProvider: Send + Sync {
    /// Stable identifier for this provider, e.g. "apps", "calculator", "emoji"
    fn id(&self) -> &'static str;

    /// Higher priority ranks first when scores tie. Defaults to 0.
    #[allow(dead_code)]
    fn priority(&self) -> i32 {
        0
    }

    /// Cheap check: should this provider attempt the given query?
    fn should_run(&self, query: &str) -> bool;

    /// Execute a search and return scored results. Called inside catch_unwind
    /// by the registry — panics are caught and logged, not propagated.
    fn search(&self, ctx: &SearchContext, query: &str) -> Vec<SearchResult>;

    /// Activate a result that was previously returned by search().
    fn activate(&self, ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError>;

    /// Reload internal state (e.g. app list, clipboard entries).
    /// Called when the overlay opens or on explicit refresh.
    fn refresh(&self) {}

    /// Update the file-index scan limits (files provider); default no-op.
    fn set_file_limits(&self, _depth: usize, _entries: usize) {}

    /// Monotonically increases after a background refresh publishes new data.
    /// The UI uses this to re-run its current query without polling providers.
    fn revision(&self) -> u64 {
        0
    }
}
