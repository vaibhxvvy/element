//! Help provider — surfaces the in-launcher docs/manual panel.
//!
//! Type `help` and press Enter to open the help panel (hotkeys, providers,
//! and tips). The UI intercepts results with `kind == "help"` and switches
//! to the help view, mirroring the settings flow.

use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider, SearchResult};

pub struct HelpProvider;

impl SearchProvider for HelpProvider {
    fn id(&self) -> &'static str {
        "help"
    }

    fn priority(&self) -> i32 {
        100
    }

    fn should_run(&self, query: &str) -> bool {
        let q = query.trim().to_ascii_lowercase();
        q == "help" || q.starts_with("help ")
    }

    fn search(&self, _ctx: &SearchContext, _query: &str) -> Vec<SearchResult> {
        vec![SearchResult {
            title: "Open Help".into(),
            subtitle: "Manual — hotkeys, commands, and tips".into(),
            kind: "help".into(),
            provider_id: "help".into(),
            action: "open".into(),
            icon_rgba: None,
            score: 1000.0,
        }]
    }

    fn activate(&self, _ctx: &SearchContext, _result: &SearchResult) -> Result<(), ElementError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::database::Database;

    #[test]
    fn matches_help_word_only() {
        let provider = HelpProvider;
        assert!(provider.should_run("help"));
        assert!(provider.should_run("HELP"));
        assert!(provider.should_run("help panel"));
        assert!(!provider.should_run("helper"));
        assert!(!provider.should_run(""));
    }

    #[test]
    fn search_returns_help_result() {
        let config = Config::default();
        let db = Database::new_in_memory();
        let ctx = SearchContext {
            config: &config,
            db: &db,
        };
        let provider = HelpProvider;
        let results = provider.search(&ctx, "help");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Open Help");
        assert_eq!(results[0].kind, "help");
        assert_eq!(results[0].score, 1000.0);
    }
}
