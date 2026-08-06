use crate::error::ElementError;
use crate::providers::SearchResult;
use crate::providers::{SearchContext, SearchProvider};

pub struct EmojiProvider;

impl SearchProvider for EmojiProvider {
    fn id(&self) -> &'static str {
        "emoji"
    }

    fn priority(&self) -> i32 {
        8
    }

    fn should_run(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.starts_with("emoji") || q.starts_with(":")
    }

    fn search(&self, ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let q = query.trim().to_lowercase();
        let term = q
            .trim_start_matches("emoji")
            .trim()
            .trim_start_matches(':')
            .trim();

        let mut results: Vec<SearchResult> = Vec::new();

        for emoji in emojis::iter() {
            let name = emoji.name().to_lowercase();
            let codes: Vec<String> = emoji.shortcodes().map(|s| s.to_string()).collect();
            if term.is_empty() || name.contains(term) || codes.iter().any(|c| c.contains(term)) {
                // Favorites (recently used emoji) get up to a 2× boost so
                // they surface above the alphabetical decay order.
                let frecency = ctx.db.emoji_frecency_score(emoji.as_str());
                let boost = 1.0 + (frecency * 5.0).min(1.0);
                results.push(SearchResult {
                    title: format!(
                        "{}  {}",
                        emoji.as_str(),
                        codes
                            .first()
                            .map(|c| format!(":{}:", c))
                            .unwrap_or_default()
                    ),
                    subtitle: emoji.name().into(),
                    kind: "emoji".into(),
                    provider_id: "emoji".into(),
                    action: emoji.as_str().into(),
                    icon_rgba: None,
                    score: (500.0 - (results.len() as f64)) * boost,
                });
                if results.len() > 20 {
                    break;
                }
            }
        }

        results
    }

    fn activate(&self, ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        if result.action.is_empty() {
            return Err(ElementError::Other("empty emoji".into()));
        }
        arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(&result.action))
            .map_err(|e| ElementError::Other(format!("clipboard error: {:?}", e)))?;
        ctx.db.record_emoji_use(&result.action);
        Ok(())
    }
}
