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
        // Bare URL: open it directly instead of searching.
        if let Some(result) = url_result(query) {
            return vec![result];
        }
        // Per-site shortcut: `yt cats`, `gh element`, ...
        if let Some(result) = prefix_search(ctx, query) {
            return vec![result];
        }
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

/// A bare URL (with scheme, `www.` host, or `host.tld/path`) becomes an
/// "open this URL" result instead of a web search. Scores above prefix
/// searches (850 vs 800) because the user typed the exact destination.
fn url_result(query: &str) -> Option<SearchResult> {
    let q = query.trim();
    if !looks_like_url(q) {
        return None;
    }
    let display = if q.len() > 64 {
        let truncated: String = q.chars().take(64).collect();
        format!("{truncated}…")
    } else {
        q.to_string()
    };
    let open = if q.contains("://") {
        q.to_string()
    } else {
        format!("https://{q}")
    };
    Some(SearchResult {
        title: format!("Open {display}"),
        subtitle: open.clone(),
        kind: "websearch".into(),
        provider_id: "websearch".into(),
        action: open,
        icon_rgba: None,
        score: 850.0,
    })
}

/// Heuristic: is this a bare URL? Scheme-prefixed URLs always qualify; bare
/// hosts need a plausible TLD (dot + 2+ alpha chars) and must not look like
/// a decimal or a file extension (`readme.md` → files provider handles it).
fn looks_like_url(q: &str) -> bool {
    if q.is_empty() || q.contains(char::is_whitespace) {
        return false;
    }
    if q.starts_with("http://")
        || q.starts_with("https://")
        || q.starts_with("ftp://")
        || q.starts_with("file://")
    {
        return true;
    }
    if q.starts_with("www.") {
        return true;
    }
    let lower = q.to_ascii_lowercase();
    const SKIP_EXTS: [&str; 14] = [
        "png", "jpg", "jpeg", "gif", "svg", "ico", "bmp", "webp", "pdf", "exe", "lnk", "zip",
        "txt", "md",
    ];
    if SKIP_EXTS.iter().any(|e| lower.ends_with(&format!(".{e}"))) {
        return false;
    }
    q.split('.').skip(1).any(|part| {
        let host = part.split('/').next().unwrap_or(part);
        host.len() >= 2 && host.chars().all(|c| c.is_ascii_alphabetic())
    })
}

/// `yt cats` → a high-ranking result for the configured YouTube template.
/// Scores well above the plain web-search row (800 vs −1) because the prefix
/// is an explicit intent, not a fallback.
fn prefix_search(ctx: &SearchContext, query: &str) -> Option<SearchResult> {
    let (prefix, rest) = query.trim().split_once(char::is_whitespace)?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let template = ctx.config.search_prefixes.get(&prefix.to_lowercase())?;
    let site = site_name(template);
    Some(SearchResult {
        title: format!("Search {} for \"{}\"", site, rest),
        subtitle: "Open in your browser".into(),
        kind: "websearch".into(),
        provider_id: "websearch".into(),
        action: template.replace("%s", &encode_query(rest)),
        icon_rgba: None,
        score: 800.0,
    })
}

/// "https://www.youtube.com/..." → "YouTube" (best effort; falls back to the
/// configured prefix key if the host can't be derived).
fn site_name(template: &str) -> String {
    let host = template
        .split("://")
        .nth(1)
        .unwrap_or(template)
        .split('/')
        .next()
        .unwrap_or(template);
    let name = host
        .strip_prefix("www.")
        .unwrap_or(host)
        .split('.')
        .next()
        .unwrap_or(host);
    match name {
        "youtube" => "YouTube".into(),
        "github" => "GitHub".into(),
        "wikipedia" => "Wikipedia".into(),
        _ => {
            let mut chars = name.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => host.to_string(),
            }
        }
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
    use super::*;
    use crate::config::Config;
    use crate::database::Database;

    #[test]
    fn encodes_query_for_url_substitution() {
        assert_eq!(encode_query("cats & dogs"), "cats%20%26%20dogs");
        assert_eq!(encode_query("a/b?c=d"), "a%2Fb%3Fc%3Dd");
    }

    #[test]
    fn prefix_query_builds_site_url() {
        let config = Config::default();
        let db = Database::new_in_memory();
        let context = SearchContext {
            config: &config,
            db: &db,
        };
        let provider = WebSearchProvider;
        let results = provider.search(&context, "yt cats & dogs");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Search YouTube for \"cats & dogs\"");
        assert_eq!(
            results[0].action,
            "https://www.youtube.com/results?search_query=cats%20%26%20dogs"
        );
        assert_eq!(results[0].score, 800.0);
    }

    #[test]
    fn prefix_case_insensitive_and_requires_term() {
        let config = Config::default();
        let db = Database::new_in_memory();
        let context = SearchContext {
            config: &config,
            db: &db,
        };
        let provider = WebSearchProvider;

        let results = provider.search(&context, "GH element");
        assert_eq!(results[0].title, "Search GitHub for \"element\"");

        // Prefix alone (no term) falls through to the plain web search.
        let results = provider.search(&context, "yt");
        assert_eq!(results[0].title, "Search web for \"yt\"");
        assert_eq!(results[0].score, -1.0);
    }

    #[test]
    fn unknown_prefix_falls_through() {
        let config = Config::default();
        let db = Database::new_in_memory();
        let context = SearchContext {
            config: &config,
            db: &db,
        };
        let provider = WebSearchProvider;
        let results = provider.search(&context, "zzz something");
        assert_eq!(results[0].title, "Search web for \"zzz something\"");
    }

    #[test]
    fn bare_url_opens_directly() {
        let config = Config::default();
        let db = Database::new_in_memory();
        let context = SearchContext {
            config: &config,
            db: &db,
        };
        let provider = WebSearchProvider;

        // With scheme
        let results = provider.search(&context, "https://github.com/vaibhxvvy/element");
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].title,
            "Open https://github.com/vaibhxvvy/element"
        );
        assert_eq!(results[0].action, "https://github.com/vaibhxvvy/element");
        assert_eq!(results[0].score, 850.0);

        // Bare host gains https://
        let results = provider.search(&context, "example.com/docs");
        assert_eq!(results[0].action, "https://example.com/docs");

        // www. host
        let results = provider.search(&context, "www.youtube.com");
        assert_eq!(results[0].action, "https://www.youtube.com");
    }

    #[test]
    fn non_urls_stay_searches() {
        let config = Config::default();
        let db = Database::new_in_memory();
        let context = SearchContext {
            config: &config,
            db: &db,
        };
        let provider = WebSearchProvider;

        // Decimals, file extensions and plain phrases are not URLs.
        for q in ["12.5", "readme.md", "cat photo", "v1.1.0", "192.168.1.1"] {
            let results = provider.search(&context, q);
            assert_eq!(results[0].score, -1.0, "query {q:?} should stay a search");
        }
    }
}
