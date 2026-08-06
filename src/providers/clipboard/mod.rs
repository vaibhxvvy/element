use crate::app::SearchResult;
use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider};

pub struct ClipboardProvider;

impl SearchProvider for ClipboardProvider {
    fn id(&self) -> &'static str {
        "clipboard"
    }

    fn priority(&self) -> i32 {
        6
    }

    fn should_run(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q == "cbhist" || q.starts_with("clip")
    }

    fn search(&self, ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let _ = query;
        let limit = ctx.config.clipboard_max_entries.max(1) as usize;
        let entries = ctx.db.load_clipboard(limit);
        entries
            .into_iter()
            .map(|(text, ts)| {
                let preview: String = text
                    .lines()
                    .next()
                    .unwrap_or(&text)
                    .chars()
                    .take(80)
                    .collect();
                SearchResult {
                    title: preview,
                    subtitle: format!("Clipboard \u{00b7} {}", ts),
                    kind: "clipboard".into(),
                    provider_id: "clipboard".into(),
                    action: text,
                    icon_rgba: None,
                    score: 200.0,
                }
            })
            .collect()
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(&result.action))
            .map_err(|e| ElementError::Other(format!("clipboard error: {:?}", e)))
    }
}
