use std::sync::Arc;

use crate::config::Config;
use crate::database::Database;
use crate::error::ElementError;
use crate::providers::SearchContext;
use crate::registry::ProviderRegistry;

#[derive(Clone)]
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

pub struct SearchEngine {
    pub config: Arc<Config>,
    pub db: Arc<Database>,
    registry: ProviderRegistry,
}

impl SearchEngine {
    pub fn new(config: Arc<Config>, db: Arc<Database>) -> Self {
        let mut registry = ProviderRegistry::new();
        registry.add(Box::new(crate::providers::apps::AppsProvider::new(
            config.search_dirs.clone(),
        )));
        registry.add(Box::new(crate::providers::calculator::CalculatorProvider));
        registry.add(Box::new(crate::providers::emoji::EmojiProvider));
        registry.add(Box::new(crate::providers::clipboard::ClipboardProvider));
        registry.add(Box::new(crate::providers::websearch::WebSearchProvider));

        Self {
            config,
            db,
            registry,
        }
    }

    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let ctx = SearchContext {
            config: &self.config,
            db: &self.db,
        };
        self.registry.search(&ctx, query)
    }

    pub fn activate(&self, result: &SearchResult) -> Result<(), ElementError> {
        let ctx = SearchContext {
            config: &self.config,
            db: &self.db,
        };
        self.registry.activate(&ctx, result)
    }

    pub fn refresh_all(&self) {
        self.registry.refresh_all();
    }

    pub fn revision(&self) -> u64 {
        self.registry.revision()
    }
}
