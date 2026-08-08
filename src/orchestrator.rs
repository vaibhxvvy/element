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
    /// Change the file-index scan limits (depth, entry cap) and re-index.
    UpdateFileIndex { depth: usize, entries: usize },
    /// Toggle the pinned state of a clipboard entry.
    PinClipboard(String),
    /// Toggle the pinned state of a clipboard image entry (by cached path).
    PinClipboardImage(String),
    /// Run a secondary action on a file/folder path.
    FileAction { path: String, action: FileAction },
}

/// Secondary actions for a file result (not the default open).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    /// Copy the path as plain text.
    CopyPath,
    /// Copy the file itself (CF_HDROP — paste into Explorer).
    CopyFile,
    /// Open the file's folder in Explorer with the file selected.
    Reveal,
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
    /// Result of [`Request::PinClipboard`]: the new pinned state.
    Pinned(bool),
}

/// Owns the config, database, and provider registry; routes requests to them.
pub struct Orchestrator {
    pub config: Arc<Config>,
    pub db: Arc<Database>,
    registry: ProviderRegistry,
}

impl Orchestrator {
    pub fn new(config: Arc<Config>, db: Arc<Database>) -> Self {
        // Seed the clipboard sort direction; the settings panel updates it
        // live without reloading the config Arc.
        crate::providers::clipboard::set_newest_first(config.clipboard_newest_first);
        let mut registry = ProviderRegistry::new();
        registry.add(Box::new(crate::providers::apps::AppsProvider::new(
            config.search_dirs.clone(),
        )));
        registry.add(Box::new(crate::providers::calculator::CalculatorProvider));
        registry.add(Box::new(crate::providers::color::ColorProvider));
        registry.add(Box::new(crate::providers::emoji::EmojiProvider));
        registry.add(Box::new(crate::providers::clipboard::ClipboardProvider));
        registry.add(Box::new(crate::providers::files::FilesProvider::new(
            config.file_search_dirs.clone(),
            config.file_index_depth,
            config.file_index_entries,
        )));
        registry.add(Box::new(crate::providers::websearch::WebSearchProvider));
        registry.add(Box::new(crate::providers::units::UnitsProvider));
        registry.add(Box::new(crate::providers::settings::SettingsProvider));
        registry.add(Box::new(crate::providers::help::HelpProvider));
        registry.add(Box::new(crate::providers::snippets::SnippetsProvider::new()));
        registry.add(Box::new(crate::providers::system::SystemProvider));

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
            Request::UpdateFileIndex { depth, entries } => {
                self.registry.update_file_limits(depth, entries);
                Outcome::Refreshed(self.revision())
            }
            Request::PinClipboard(text) => Outcome::Pinned(self.db.toggle_clipboard_pinned(&text)),
            Request::PinClipboardImage(path) => {
                Outcome::Pinned(self.db.toggle_clipboard_image_pinned(&path))
            }
            Request::FileAction { path, action } => {
                Outcome::Activated(self.file_action(&path, action))
            }
        }
    }

    /// Run a secondary file action (copy path, copy file, reveal).
    fn file_action(&self, path: &str, action: FileAction) -> Result<(), ElementError> {
        match action {
            FileAction::CopyPath => arboard::Clipboard::new()
                .and_then(|mut c| c.set_text(path))
                .map_err(|e| ElementError::Other(format!("clipboard error: {:?}", e))),
            FileAction::CopyFile => crate::platform::copy_files_to_clipboard(&[path.to_string()])
                .map_err(ElementError::Other),
            FileAction::Reveal => std::process::Command::new("explorer.exe")
                .arg("/select,")
                .arg(path)
                .spawn()
                .map(|_| ())
                .map_err(ElementError::Io),
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
