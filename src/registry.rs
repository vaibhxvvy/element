use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::app::SearchResult;
use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider};

/// Holds all registered SearchProviders and dispatches search/activate/refresh
/// across them. Each provider's search() and activate() calls are wrapped in
/// catch_unwind so a buggy provider never brings down the whole overlay.
pub struct ProviderRegistry {
    providers: Vec<Box<dyn SearchProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn add(&mut self, provider: Box<dyn SearchProvider>) {
        self.providers.push(provider);
    }

    /// Iterate providers whose should_run() returns true, collect their results
    /// inside catch_unwind, and return them sorted by score descending.
    pub fn search(&self, ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = Vec::new();

        for provider in &self.providers {
            if !provider.should_run(query) {
                continue;
            }
            let id = provider.id();
            let result = catch_unwind(AssertUnwindSafe(|| provider.search(ctx, query)));
            match result {
                Ok(mut r) => results.append(&mut r),
                Err(_) => {
                    eprintln!(
                        "[element] provider '{id}' panicked during search — results dropped"
                    );
                }
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

    /// Find the provider that owns `result` and call its activate() inside
    /// catch_unwind.
    pub fn activate(
        &self,
        ctx: &SearchContext,
        result: &SearchResult,
    ) -> Result<(), ElementError> {
        for provider in &self.providers {
            if provider.id() != result.provider_id {
                continue;
            }
            let action = catch_unwind(AssertUnwindSafe(|| provider.activate(ctx, result)));
            return match action {
                Ok(r) => r,
                Err(_) => {
                    let msg = format!("provider '{}' panicked during activate", provider.id());
                    eprintln!("[element] {msg}");
                    Err(ElementError::Provider {
                        provider: provider.id().to_string(),
                        detail: "panic during activate".into(),
                    })
                }
            };
        }
        Err(ElementError::Other(format!(
            "no provider registered with id '{}'",
            result.provider_id
        )))
    }

    /// Call refresh() on every provider. The default implementation is a no-op;
    /// only providers that need to reload state (e.g. AppsProvider) override it.
    pub fn refresh_all(&self) {
        for provider in &self.providers {
            let id = provider.id();
            let result = catch_unwind(AssertUnwindSafe(|| provider.refresh()));
            if result.is_err() {
                eprintln!("[element] provider '{id}' panicked during refresh");
            }
        }
    }
}
