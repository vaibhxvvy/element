use crate::app::SearchResult;
use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider};

pub struct WebSearchProvider;

impl SearchProvider for WebSearchProvider {
    fn id(&self) -> &'static str {
        "websearch"
    }

    /// Lowest priority so web search always appears at the bottom.
    fn priority(&self) -> i32 {
        -100
    }

    fn should_run(&self, query: &str) -> bool {
        !query.trim().is_empty()
    }

    fn search(&self, ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let url = config_or_default(&ctx.config.search_url, "https://duckduckgo.com/search?q=%s");
        vec![SearchResult {
            title: format!("Search web for \"{}\"", query),
            subtitle: url.replace("%s", query),
            kind: "websearch".into(),
            provider_id: "websearch".into(),
            icon_rgba: None,
            score: -1.0,
        }]
    }

    fn activate(
        &self,
        _ctx: &SearchContext,
        result: &SearchResult,
    ) -> Result<(), ElementError> {
        webbrowser::open(&result.subtitle)
            .map_err(|e| ElementError::Other(format!("browser error: {:?}", e)))
    }
}

fn config_or_default(config: &str, default: &str) -> String {
    if config.is_empty() {
        default.into()
    } else {
        config.into()
    }
}
