use crate::error::ElementError;
use crate::providers::SearchResult;
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
        let search_url = url.replace("%s", &encode_query(query));
        vec![SearchResult {
            title: format!("Search web for \"{}\"", query),
            subtitle: "Open in your browser".into(),
            kind: "websearch".into(),
            provider_id: "websearch".into(),
            action: search_url,
            icon_rgba: None,
            score: -1.0,
        }]
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        webbrowser::open(&result.action)
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

fn encode_query(query: &str) -> String {
    let mut encoded = String::with_capacity(query.len());
    for byte in query.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::encode_query;

    #[test]
    fn encodes_query_for_url_substitution() {
        assert_eq!(encode_query("cats & dogs"), "cats%20%26%20dogs");
        assert_eq!(encode_query("a/b?c=d"), "a%2Fb%3Fc%3Dd");
    }
}
