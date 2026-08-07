use crate::error::ElementError;
use crate::providers::SearchResult;
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

        // Text entries + image entries, merged by (pinned, recency). The
        // created_at strings are ISO-ish and sortable lexicographically.
        let mut merged: Vec<(bool, String, SearchResult)> = Vec::new();
        for (text, ts, pinned) in ctx.db.load_clipboard(limit) {
            let preview: String = text
                .lines()
                .next()
                .unwrap_or(&text)
                .chars()
                .take(80)
                .collect();
            merged.push((
                pinned,
                ts.clone(),
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
        for (path, width, height, ts, pinned) in ctx.db.load_clipboard_images(limit) {
            let thumb = thumbnail_rgba(&path);
            let title = format!("Image \u{00b7} {width}\u{00d7}{height}");
            merged.push((
                pinned,
                ts.clone(),
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
            b.0.cmp(&a.0)
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.2.title.cmp(&b.2.title))
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
