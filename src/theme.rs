use iced::Color;

/// Central place for all colour, spacing, radius, and typography tokens.
/// Keeps the UI module focused on layout and the actual tokens easy to
/// tweak without hunting through inline values.

// --- Backgrounds ---
pub const BG_PRIMARY: Color = Color::WHITE;
pub const BG_SELECTED: Color = Color::from_rgb(235.0 / 255.0, 235.0 / 255.0, 245.0 / 255.0);
pub const BG_INPUT: Color = Color::from_rgb(245.0 / 255.0, 245.0 / 255.0, 245.0 / 255.0);

// --- Text ---
pub const TEXT_PRIMARY: Color = Color::from_rgb(30.0 / 255.0, 30.0 / 255.0, 35.0 / 255.0);
pub const TEXT_MUTED: Color = Color::from_rgb(100.0 / 255.0, 100.0 / 255.0, 110.0 / 255.0);
pub const TEXT_PLACEHOLDER: Color =
    Color::from_rgb(160.0 / 255.0, 160.0 / 255.0, 168.0 / 255.0);
pub const TEXT_ICON: Color = Color::from_rgb(120.0 / 255.0, 120.0 / 255.0, 128.0 / 255.0);

// --- Accent ---
pub const ACCENT: Color = Color::from_rgb(150.0 / 255.0, 150.0 / 255.0, 255.0 / 255.0);

// --- Sizing ---
pub const RESULT_HEIGHT: f32 = 42.0;
pub const ICON_SIZE: f32 = 16.0;
pub const INPUT_PADDING_TOP: f32 = 14.0;
pub const INPUT_PADDING_SIDES: f32 = 16.0;
pub const CONTENT_PADDING_SIDES: f32 = 16.0;
pub const SPACING_SM: f32 = 1.0;
pub const SPACING_MD: f32 = 12.0;

// --- Layout ---
pub const MIN_WINDOW_HEIGHT: f32 = 56.0;
pub const MAX_WINDOW_HEIGHT: f32 = 500.0;
pub const SEARCH_BAR_HEIGHT: f32 = 52.0;
pub const BOTTOM_PADDING: f32 = 8.0;
pub const MAX_VISIBLE_RESULTS: usize = 10;

// --- Typography ---
pub const TITLE_SIZE: f32 = 13.0;
pub const SUBTITLE_SIZE: f32 = 11.0;

// --- Borders ---
pub const BORDER_RADIUS: f32 = 0.0;
pub const BORDER_WIDTH: f32 = 0.0;

// --- Selection indicator ---
pub const INDICATOR_WIDTH: f32 = 3.0;
