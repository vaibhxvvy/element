//! Settings provider — surfaces the settings panel entry point.
//!
//! Type `settings` and press Enter to open the in-launcher settings panel
//! (window width, search engine, accent color, autostart). The UI intercepts
//! results with `kind == "settings"` and switches to the settings view.

use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider, SearchResult};

pub struct SettingsProvider;

impl SearchProvider for SettingsProvider {
    fn id(&self) -> &'static str {
        "settings"
    }

    fn priority(&self) -> i32 {
        100
    }

    fn should_run(&self, query: &str) -> bool {
        let q = query.trim().to_ascii_lowercase();
        q == "settings" || q.starts_with("settings ")
    }

    fn search(&self, _ctx: &SearchContext, _query: &str) -> Vec<SearchResult> {
        vec![SearchResult {
            title: "Open Settings".into(),
            subtitle: "Configure Element — width, search engine, accent, autostart".into(),
            kind: "settings".into(),
            provider_id: "settings".into(),
            action: "open".into(),
            icon_rgba: None,
            score: 1000.0,
        }]
    }

    fn activate(&self, _ctx: &SearchContext, _result: &SearchResult) -> Result<(), ElementError> {
        Ok(())
    }
}
