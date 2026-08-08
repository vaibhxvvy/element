use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::ElementError;
use crate::providers::SearchResult;
use crate::providers::{SearchContext, SearchProvider};

/// Clipboard history sort direction, swapped live from the settings panel
/// (the running `Arc<Config>` is not reloaded when settings are saved, so
/// this is the source of truth for the provider).
static NEWEST_FIRST: AtomicBool = AtomicBool::new(true);

/// Change the clipboard history order; called from the settings panel and at
/// startup from the config.
pub fn set_newest_first(newest: bool) {
    NEWEST_FIRST.store(newest, Ordering::Relaxed);
}

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
        let limit = ctx.config.clipboard_max_entries.max(1) as usize;

        // Query grammar (tokens after the `clip`/`cbhist` trigger):
        //   clip <text…>          — filter entries containing the text
        //   clip today            — only entries captured today
        //   clip yesterday        — only entries captured yesterday
        //   clip 2026-08-05       — only entries captured on that date
        //   clip last7d           — entries from the last 7 days
        //   clip sort:new         — newest first (override)
        //   clip sort:old         — oldest first (override)
        let mut newest_first = NEWEST_FIRST.load(Ordering::Relaxed);
        let mut text_parts: Vec<&str> = Vec::new();
        let mut date: Option<DateFilter> = None;
        for word in query.split_whitespace().skip(1) {
            let lower = word.to_lowercase();
            match lower.as_str() {
                "sort:new" => newest_first = true,
                "sort:old" => newest_first = false,
                "today" => date = Some(DateFilter::Today),
                "yesterday" => date = Some(DateFilter::Yesterday),
                w if is_iso_date(w) => date = Some(DateFilter::Exact(w.to_string())),
                w if parse_last_days(w).is_some() => {
                    date = Some(DateFilter::LastDays(parse_last_days(w).unwrap()))
                }
                _ => text_parts.push(word),
            }
        }

        let text_like = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(" "))
        };
        let (date_from, date_to) = match date {
            Some(DateFilter::Today) => (ctx.db.local_date(0), ctx.db.local_date(0)),
            Some(DateFilter::Yesterday) => (ctx.db.local_date(-1), ctx.db.local_date(-1)),
            Some(DateFilter::Exact(d)) => (d.clone(), d),
            Some(DateFilter::LastDays(days)) => (ctx.db.local_date(-days), ctx.db.local_date(0)),
            None => (String::new(), String::new()),
        };
        let date_from = (!date_from.is_empty()).then_some(date_from.as_str());
        let date_to = (!date_to.is_empty()).then_some(date_to.as_str());

        // Text + image entries merged by (pinned, id). `id` is the stable
        // capture order — two captures within the same second share a
        // `created_at`, and sorting by timestamp alone could put the newest
        // entry below an older one.
        let mut merged: Vec<(bool, i64, SearchResult)> = Vec::new();
        for (text, ts, pinned, id) in ctx.db.load_clipboard_filtered(
            limit,
            text_like.as_deref(),
            date_from,
            date_to,
            newest_first,
        ) {
            let preview: String = text
                .lines()
                .next()
                .unwrap_or(&text)
                .chars()
                .take(80)
                .collect();
            merged.push((
                pinned,
                id,
                SearchResult {
                    title: if pinned {
                        format!("\u{1f4cc} {}", preview)
                    } else {
                        preview
                    },
                    subtitle: format!(
                        "Clipboard \u{00b7} {}{}",
                        ts,
                        if pinned { " \u{00b7} pinned" } else { "" }
                    ),
                    kind: "clipboard".into(),
                    provider_id: "clipboard".into(),
                    action: text,
                    icon_rgba: None,
                    score: 200.0,
                },
            ));
        }
        for (path, width, height, ts, pinned, id) in
            ctx.db
                .load_clipboard_images_filtered(limit, date_from, date_to, newest_first)
        {
            let thumb = thumbnail_rgba(&path);
            let title = format!("Image \u{00b7} {width}\u{00d7}{height}");
            merged.push((
                pinned,
                id,
                SearchResult {
                    title: if pinned {
                        format!("\u{1f4cc} {}", title)
                    } else {
                        title
                    },
                    subtitle: format!(
                        "Clipboard \u{00b7} {}{}",
                        ts,
                        if pinned { " \u{00b7} pinned" } else { "" }
                    ),
                    kind: "clipboard-image".into(),
                    provider_id: "clipboard".into(),
                    action: path,
                    icon_rgba: thumb,
                    score: 200.0,
                },
            ));
        }
        merged.sort_by(|a, b| {
            b.0.cmp(&a.0).then_with(|| {
                if newest_first {
                    b.1.cmp(&a.1)
                } else {
                    a.1.cmp(&b.1)
                }
            })
        });
        merged.into_iter().map(|(_, _, result)| result).collect()
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        if result.kind == "clipboard-image" {
            // Restore the image to the clipboard as CF_DIB (32 bpp top-down
            // BI_RGB), so a paste lands the original pixels.
            let img = image::open(&result.action)
                .map_err(|e| ElementError::Other(format!("image error: {e:?}")))?
                .to_rgba8();
            let dib = crate::platform::rgba_to_dib(img.as_raw(), img.width(), img.height());
            return crate::platform::set_clipboard_bitmap(
                &dib,
                crate::platform::ClipboardBitmapFormat::Dib,
            )
            .map_err(ElementError::Other);
        }
        arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(&result.action))
            .map_err(|e| ElementError::Other(format!("clipboard error: {:?}", e)))
    }
}

/// Which calendar window the query asked for.
enum DateFilter {
    Today,
    Yesterday,
    /// Exact `YYYY-MM-DD`.
    Exact(String),
    /// `last Nd` — the trailing N days including today.
    LastDays(i64),
}

/// `YYYY-MM-DD` — cheap shape check, no date library.
fn is_iso_date(word: &str) -> bool {
    let b = word.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && word[..4].parse::<u16>().is_ok()
        && word[5..7].parse::<u8>().is_ok()
        && word[8..10].parse::<u8>().is_ok()
}

/// `last7d` / `last30d` → number of days; anything else → `None`.
fn parse_last_days(word: &str) -> Option<i64> {
    let days = word
        .strip_prefix("last")?
        .strip_suffix('d')?
        .parse::<i64>()
        .ok()?;
    (days > 0).then_some(days)
}

/// Decode the cached 64×64 thumbnail for an image entry into RGBA for
/// `icon_rgba`. The thumbnail lives next to the full image:
/// `<full>.png` → `<stem>-thumb.png`.
fn thumbnail_rgba(full_path: &str) -> Option<(Vec<u8>, u32, u32)> {
    let path = std::path::Path::new(full_path);
    let stem = path.file_stem()?.to_string_lossy();
    let dir = path.parent()?;
    let thumb = dir.join(format!("{stem}-thumb.png"));
    let img = image::open(thumb).ok()?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    Some((img.into_raw(), w, h))
}

#[cfg(test)]
mod tests {
    use super::{is_iso_date, parse_last_days, set_newest_first, ClipboardProvider};
    use crate::config::Config;
    use crate::database::Database;
    use crate::providers::{SearchContext, SearchProvider};

    fn ctx<'a>(db: &'a Database, config: &'a Config) -> SearchContext<'a> {
        SearchContext { config, db }
    }

    #[test]
    fn date_helpers() {
        assert!(is_iso_date("2026-08-05"));
        assert!(!is_iso_date("05-08-2026"));
        assert!(!is_iso_date("2026-8-5"));
        assert!(!is_iso_date("tomorrow"));
        assert_eq!(parse_last_days("last7d"), Some(7));
        assert_eq!(parse_last_days("last30d"), Some(30));
        assert_eq!(parse_last_days("last1d"), Some(1));
        assert_eq!(parse_last_days("last7h"), None);
        assert_eq!(parse_last_days("week"), None);
    }

    #[test]
    fn newest_first_and_oldest_first_order() {
        let db = Database::new_in_memory();
        for i in 0..4 {
            db.save_clipboard(&format!("entry {i}"), 100);
        }
        let config = Config::default();
        let provider = ClipboardProvider;

        set_newest_first(true);
        let results = provider.search(&ctx(&db, &config), "clip");
        assert_eq!(results[0].title, "entry 3", "newest on top");
        assert_eq!(results[3].title, "entry 0");

        set_newest_first(false);
        let results = provider.search(&ctx(&db, &config), "clip");
        assert_eq!(results[0].title, "entry 0", "oldest first");
        assert_eq!(results[3].title, "entry 3");

        // Query-level override beats the global.
        set_newest_first(false);
        let results = provider.search(&ctx(&db, &config), "clip sort:new");
        assert_eq!(results[0].title, "entry 3");
        set_newest_first(true);
    }

    #[test]
    fn text_filter_matches_only_containing_entries() {
        let db = Database::new_in_memory();
        db.save_clipboard("rust code sample", 100);
        db.save_clipboard("cooking pasta", 100);
        db.save_clipboard("rust playground", 100);
        let config = Config::default();
        let provider = ClipboardProvider;
        let results = provider.search(&ctx(&db, &config), "clip rust");
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.title.contains("rust")));
        let none = provider.search(&ctx(&db, &config), "clip banana");
        assert!(none.is_empty());
    }

    #[test]
    fn date_filter_today_matches_fresh_captures() {
        let db = Database::new_in_memory();
        db.save_clipboard("captured just now", 100);
        let config = Config::default();
        let provider = ClipboardProvider;
        let results = provider.search(&ctx(&db, &config), "clip today");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "captured just now");
        let none = provider.search(&ctx(&db, &config), "clip 1999-01-01");
        assert!(none.is_empty());
    }

    #[test]
    fn images_merge_and_obey_sort() {
        let db = Database::new_in_memory();
        db.save_clipboard_image("hash-img", "C:\\cache\\img.png", 10, 10, 100);
        db.save_clipboard("text after image", 100);
        let config = Config::default();
        let provider = ClipboardProvider;
        set_newest_first(true);
        let results = provider.search(&ctx(&db, &config), "clip");
        assert_eq!(
            results[0].title, "text after image",
            "newest capture on top"
        );
        assert!(results.iter().any(|r| r.kind == "clipboard-image"));
        set_newest_first(true);
    }
}
