use crate::app::SearchResult;
use crate::config::Config;
use crate::database::Database;
use crate::error::ElementError;

pub mod apps;
pub mod calculator;
pub mod clipboard;
pub mod emoji;
pub mod websearch;

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
    fn activate(
        &self,
        ctx: &SearchContext,
        result: &SearchResult,
    ) -> Result<(), ElementError>;

    /// Reload internal state (e.g. app list, clipboard entries).
    /// Called when the overlay opens or on explicit refresh.
    fn refresh(&self) {}
}
