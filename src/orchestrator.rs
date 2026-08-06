//! The **Orchestrator** — the single entry point that takes a user request and
//! performs it.
//!
//! A user types or clicks → the UI builds a [`Request`] → [`Orchestrator::handle`]
//! routes it to the [`ProviderRegistry`], which fans out to every
//! [`SearchProvider`](crate::providers::SearchProvider) inside `catch_unwind`.
//!
//! ```
//! let outcome = orchestrator.handle(Request::Search("note".into()));
//! if let Outcome::Results(results) = outcome { /* show them */ }
//! ```

use std::sync::Arc;

use crate::config::Config;
use crate::database::Database;
use crate::error::ElementError;
use crate::providers::{SearchContext, SearchResult};
use crate::registry::ProviderRegistry;

/// A user request the orchestrator can act on.
#[derive(Debug, Clone)]
pub enum Request {
    /// Run a query across all providers and return merged, scored results.
    Search(String),
    /// Perform the chosen result (launch an app, copy text, open a URL, ...).
    Activate(SearchResult),
    /// Ask every provider to reload its index (app scan, clipboard, ...).
    Refresh,
}

/// The outcome of performing a [`Request`].
#[derive(Debug)]
pub enum Outcome {
    /// Results for a [`Request::Search`], sorted by score descending.
    Results(Vec<SearchResult>),
    /// Result of a [`Request::Activate`].
    Activated(Result<(), ElementError>),
    /// Result of a [`Request::Refresh`]: the new provider data revision.
    Refreshed(u64),
}

/// Owns the config, database, and provider registry; routes requests to them.
pub struct Orchestrator {
    pub config: Arc<Config>,
    pub db: Arc<Database>,
    registry: ProviderRegistry,
}

impl Orchestrator {
    pub fn new(config: Arc<Config>, db: Arc<Database>) -> Self {
        let mut registry = ProviderRegistry::new();
        registry.add(Box::new(crate::providers::apps::AppsProvider::new(
            config.search_dirs.clone(),
        )));
        registry.add(Box::new(crate::providers::calculator::CalculatorProvider));
        registry.add(Box::new(crate::providers::emoji::EmojiProvider));
        registry.add(Box::new(crate::providers::clipboard::ClipboardProvider));
        registry.add(Box::new(crate::providers::files::FilesProvider::new(
            config.file_search_dirs.clone(),
        )));
        registry.add(Box::new(crate::providers::websearch::WebSearchProvider));
        registry.add(Box::new(crate::providers::units::UnitsProvider));
        registry.add(Box::new(crate::providers::settings::SettingsProvider));

        Self {
            config,
            db,
            registry,
        }
    }

    /// Take a user request and perform it. This is the orchestrator's one entry
    /// point — the UI never touches providers directly.
    pub fn handle(&self, request: Request) -> Outcome {
        match request {
            Request::Search(query) => Outcome::Results(self.search(&query)),
            Request::Activate(result) => Outcome::Activated(self.activate(&result)),
            Request::Refresh => {
                self.refresh_all();
                Outcome::Refreshed(self.revision())
            }
        }
    }

    /// Run `query` through every provider and return merged results.
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let ctx = SearchContext {
            config: &self.config,
            db: &self.db,
        };
        self.registry.search(&ctx, query)
    }

    /// Perform `result` (launch, copy, open browser, ...).
    pub fn activate(&self, result: &SearchResult) -> Result<(), ElementError> {
        let ctx = SearchContext {
            config: &self.config,
            db: &self.db,
        };
        self.registry.activate(&ctx, result)
    }

    /// Ask every provider to refresh its index (non-blocking where possible).
    pub fn refresh_all(&self) {
        self.registry.refresh_all();
    }

    /// Latest published data revision across all providers.
    pub fn revision(&self) -> u64 {
        self.registry.revision()
    }
}
