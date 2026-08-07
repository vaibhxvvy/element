//! Color picker provider — `#ff0000` → swatch + copyable variants.
//!
//! Type a hex color with a `#` prefix (`#f00`, `#ff0000`, `#ff000080`) and
//! Enter copies the hex value; the result list also offers `rgb(...)` and
//! `hsl(...)` variants. The swatch is rendered through `icon_rgba` (32×32
//! solid fill), which the UI draws as the result icon.

use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider, SearchResult};

/// 32×32 solid RGBA swatch for a color.
fn swatch(r: u8, g: u8, b: u8) -> (Vec<u8>, u32, u32) {
    let mut pixels = Vec::with_capacity(32 * 32 * 4);
    for _ in 0..(32 * 32) {
        pixels.extend_from_slice(&[r, g, b, 255]);
    }
    (pixels, 32, 32)
}

/// Parse a `#rgb` / `#rrggbb` / `#rrggbbaa` query into (r, g, b). Returns
/// None for anything that is not exactly a hex color.
fn parse_hex(query: &str) -> Option<(u8, u8, u8)> {
    let hex = query.trim().strip_prefix('#')?;
    if hex.len() != 3 && hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let expand = |pair: &str| u8::from_str_radix(pair, 16).ok();
    match hex.len() {
        3 => {
            let r = expand(&hex[0..1])? * 17;
            let g = expand(&hex[1..2])? * 17;
            let b = expand(&hex[2..3])? * 17;
            Some((r, g, b))
        }
        6 => Some((
            expand(&hex[0..2])?,
            expand(&hex[2..4])?,
            expand(&hex[4..6])?,
        )),
        8 => Some((
            expand(&hex[0..2])?,
            expand(&hex[2..4])?,
            expand(&hex[4..6])?,
        )),
        _ => None,
    }
}

/// `#ff0000` → "255" (rgb strings use plain decimals).
fn to_rgb(r: u8, g: u8, b: u8) -> String {
    format!("rgb({}, {}, {})", r, g, b)
}

/// Simple HSL conversion (CSS-style percentages, hue in degrees).
fn to_hsl(r: u8, g: u8, b: u8) -> String {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, s) = if d == 0.0 {
        (0.0, 0.0)
    } else {
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == rf {
            (gf - bf) / d + if gf < bf { 6.0 } else { 0.0 }
        } else if max == gf {
            (bf - rf) / d + 2.0
        } else {
            (rf - gf) / d + 4.0
        };
        (h * 60.0, s)
    };
    format!("hsl({:.0}°, {:.0}%, {:.0}%)", h, s * 100.0, l * 100.0)
}

pub struct ColorProvider;

impl SearchProvider for ColorProvider {
    fn id(&self) -> &'static str {
        "color"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn should_run(&self, query: &str) -> bool {
        let q = query.trim();
        q.starts_with('#') && parse_hex(q).is_some()
    }

    fn search(&self, _ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let Some((r, g, b)) = parse_hex(query) else {
            return Vec::new();
        };
        let hex = format!("#{:02X}{:02X}{:02X}", r, g, b);
        let icon = Some(swatch(r, g, b));
        vec![
            SearchResult {
                title: hex.clone(),
                subtitle: "Copy hex".into(),
                kind: "color".into(),
                provider_id: "color".into(),
                action: hex,
                icon_rgba: icon.clone(),
                score: 950.0,
            },
            SearchResult {
                title: to_rgb(r, g, b),
                subtitle: "Copy RGB".into(),
                kind: "color".into(),
                provider_id: "color".into(),
                action: to_rgb(r, g, b),
                icon_rgba: icon.clone(),
                score: 940.0,
            },
            SearchResult {
                title: to_hsl(r, g, b),
                subtitle: "Copy HSL".into(),
                kind: "color".into(),
                provider_id: "color".into(),
                action: to_hsl(r, g, b),
                icon_rgba: icon,
                score: 930.0,
            },
        ]
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| ElementError::Other(format!("clipboard error: {e:?}")))?;
        clipboard
            .set_text(result.action.clone())
            .map_err(|e| ElementError::Other(format!("clipboard error: {e:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::database::Database;

    fn ctx<'a>(config: &'a Config, db: &'a Database) -> SearchContext<'a> {
        SearchContext { config, db }
    }

    #[test]
    fn parses_hex_forms() {
        assert_eq!(parse_hex("#f00"), Some((255, 0, 0)));
        assert_eq!(parse_hex("#ff0000"), Some((255, 0, 0)));
        assert_eq!(parse_hex("#00ff00"), Some((0, 255, 0)));
        assert_eq!(parse_hex("#0000ff80"), Some((0, 0, 255)));
        assert_eq!(parse_hex("#abc"), Some((170, 187, 204)));
        assert_eq!(parse_hex("#nothex"), None);
        assert_eq!(parse_hex("#ff00"), None);
        assert_eq!(parse_hex("ff0000"), None);
    }

    #[test]
    fn should_run_only_on_hash_colors() {
        let provider = ColorProvider;
        assert!(provider.should_run("#fff"));
        assert!(provider.should_run("#FF0000"));
        assert!(!provider.should_run("ff0000"));
        assert!(!provider.should_run("#zzz"));
    }

    #[test]
    fn search_returns_three_variants_with_swatch() {
        let config = Config::default();
        let db = Database::new_in_memory();
        let context = ctx(&config, &db);
        let provider = ColorProvider;
        let results = provider.search(&context, "#ff0000");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].title, "#FF0000");
        assert_eq!(results[0].action, "#FF0000");
        assert_eq!(results[1].title, "rgb(255, 0, 0)");
        assert!(results[0].icon_rgba.is_some());
        assert_eq!(results[0].score, 950.0);
    }

    #[test]
    fn hsl_conversion_is_css_like() {
        assert_eq!(to_hsl(255, 0, 0), "hsl(0°, 100%, 50%)");
        assert_eq!(to_hsl(0, 0, 255), "hsl(240°, 100%, 50%)");
        assert_eq!(to_hsl(0, 128, 0), "hsl(120°, 100%, 25%)");
    }
}
