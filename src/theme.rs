use std::sync::Mutex;

use iced::Color;

// Central place for all colour, spacing, radius, and typography tokens.
// Design: dark launcher with DWM acrylic blur (60px), 50% opacity #3c3c3c
// background, #4d4d4d full-opacity 2px inset border, rounded corners.

/// The default accent when no config override is set.
const ACCENT_DEFAULT: Color = Color::from_rgb(86.0 / 255.0, 156.0 / 255.0, 214.0 / 255.0);

static ACCENT: Mutex<Color> = Mutex::new(ACCENT_DEFAULT);

/// Current accent color — configurable via the settings panel.
pub fn accent() -> Color {
    *ACCENT.lock().expect("accent mutex poisoned")
}

/// Override the accent color (settings panel).
pub fn set_accent(color: Color) {
    *ACCENT.lock().expect("accent mutex poisoned") = color;
}

/// Parse `"#rrggbb"` into a [`Color`]. Returns `None` for malformed input.
pub fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::from_rgb(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
    ))
}

/// Apply the configured accent from config (`accent = "#rrggbb"`).
pub fn apply_config_accent(configured: &str) {
    if let Some(color) = parse_hex_color(configured) {
        set_accent(color);
    }
}

// --- Backgrounds ---
/// Main container background.
pub const BG_PRIMARY: Color = Color::from_rgb(60.0 / 255.0, 60.0 / 255.0, 60.0 / 255.0);
/// Selected result row highlight
pub const BG_SELECTED: Color = Color::from_rgb(77.0 / 255.0, 77.0 / 255.0, 77.0 / 255.0);
/// Text input background — slightly darker to distinguish from results
pub const BG_INPUT: Color = Color::from_rgb(30.0 / 255.0, 30.0 / 255.0, 30.0 / 255.0);

// --- Text ---
pub const TEXT_PRIMARY: Color = Color::from_rgb(220.0 / 255.0, 220.0 / 255.0, 220.0 / 255.0);
pub const TEXT_MUTED: Color = Color::from_rgb(160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0);
pub const TEXT_PLACEHOLDER: Color = Color::from_rgb(140.0 / 255.0, 140.0 / 255.0, 140.0 / 255.0);
pub const TEXT_ICON: Color = TEXT_MUTED;
pub const TEXT_ERROR: Color = Color::from_rgb(1.0, 100.0 / 255.0, 100.0 / 255.0);

// --- Accent ---
/// #4d4d4d full-opacity border
pub const BORDER: Color = Color::from_rgb(77.0 / 255.0, 77.0 / 255.0, 77.0 / 255.0);

// --- Sizing ---
/// Searchbar and each result row are equal; footer takes the remainder.
/// Derived from the window ratio: searchbar + 1 result + footer = width / 4.
pub const RESULT_HEIGHT: f32 = (WINDOW_WIDTH / 4.0 - FOOTER_HEIGHT) / 2.0;
pub const ICON_SIZE: f32 = 32.0;
pub const INPUT_PADDING_VERTICAL: f32 = 35.0;
pub const INPUT_PADDING_SIDES: f32 = 12.0;
pub const HEADER_PADDING_VERTICAL: f32 = 0.0;
pub const CONTENT_PADDING_SIDES: f32 = 12.0;
pub const SPACING_SM: f32 = 2.0;
pub const SPACING_MD: f32 = 8.0;

// --- Layout ---
/// Fixed launcher width (960 px).
pub const WINDOW_WIDTH: f32 = 960.0;
/// Searchbar + 1 result + footer at the 100:25 width ratio.
pub const TOTAL_ONE_RESULT_HEIGHT: f32 = WINDOW_WIDTH / 4.0;
pub const MIN_WINDOW_HEIGHT: f32 = TOTAL_ONE_RESULT_HEIGHT;
pub const MAX_WINDOW_HEIGHT: f32 = 500.0;
pub const SEARCH_BAR_HEIGHT: f32 = RESULT_HEIGHT;
pub const FOOTER_HEIGHT: f32 = 56.0;
pub const MAX_VISIBLE_RESULTS: usize = 10;

// --- Typography ---
pub const TITLE_SIZE: f32 = 16.0;
pub const SUBTITLE_SIZE: f32 = 13.0;

// --- Borders ---
/// Input field corner radius
pub const BORDER_RADIUS: f32 = 6.0;
/// Main container corner radius (rounded corners)
pub const CONTAINER_RADIUS: f32 = 12.0;
/// #4d4d4d full-opacity 2px inset border
pub const BORDER_WIDTH: f32 = 2.0;

// --- Selection indicator ---
pub const INDICATOR_WIDTH: f32 = 3.0;
