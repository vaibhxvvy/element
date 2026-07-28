use iced::Color;

// Central place for all colour, spacing, radius, and typography tokens.
// Design: dark launcher with DWM acrylic blur (60px), 50% opacity #3c3c3c
// background, #4d4d4d full-opacity 2px inset border, rounded corners.

// --- Backgrounds ---
/// Main container: semi-transparent dark to let DWM acrylic show through.
/// When DWM acrylic is unavailable, this blends with Theme::Dark background.
pub const BG_PRIMARY: Color = Color::from_rgba(60.0 / 255.0, 60.0 / 255.0, 60.0 / 255.0, 0.35);
/// Selected result row highlight
pub const BG_SELECTED: Color = Color::from_rgba(77.0 / 255.0, 77.0 / 255.0, 77.0 / 255.0, 0.5);
/// Text input background — slightly darker to distinguish from results
pub const BG_INPUT: Color = Color::from_rgba(30.0 / 255.0, 30.0 / 255.0, 30.0 / 255.0, 0.4);

// --- Text ---
pub const TEXT_PRIMARY: Color = Color::from_rgb(220.0 / 255.0, 220.0 / 255.0, 220.0 / 255.0);
pub const TEXT_MUTED: Color = Color::from_rgb(160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0);
pub const TEXT_PLACEHOLDER: Color = Color::from_rgb(140.0 / 255.0, 140.0 / 255.0, 140.0 / 255.0);
pub const TEXT_ICON: Color = TEXT_MUTED;
pub const TEXT_ERROR: Color = Color::from_rgb(1.0, 100.0 / 255.0, 100.0 / 255.0);

// --- Accent ---
pub const ACCENT: Color = Color::from_rgb(86.0 / 255.0, 156.0 / 255.0, 214.0 / 255.0);
/// #4d4d4d full-opacity border
pub const BORDER: Color = Color::from_rgb(77.0 / 255.0, 77.0 / 255.0, 77.0 / 255.0);

// --- Sizing ---
pub const RESULT_HEIGHT: f32 = 42.0;
pub const ICON_SIZE: f32 = 18.0;
pub const INPUT_PADDING_VERTICAL: f32 = 10.0;
pub const INPUT_PADDING_SIDES: f32 = 12.0;
pub const HEADER_PADDING_VERTICAL: f32 = 0.0;
pub const CONTENT_PADDING_SIDES: f32 = 12.0;
pub const SPACING_SM: f32 = 2.0;
pub const SPACING_MD: f32 = 8.0;

// --- Layout ---
pub const MIN_WINDOW_HEIGHT: f32 = 52.0;
pub const MAX_WINDOW_HEIGHT: f32 = 500.0;
pub const SEARCH_BAR_HEIGHT: f32 = 48.0;
pub const BOTTOM_PADDING: f32 = 8.0;
pub const STATUS_HEIGHT: f32 = 24.0;
pub const MAX_VISIBLE_RESULTS: usize = 10;

// --- Typography ---
pub const TITLE_SIZE: f32 = 13.0;
pub const SUBTITLE_SIZE: f32 = 11.0;

// --- Borders ---
/// Input field corner radius
pub const BORDER_RADIUS: f32 = 6.0;
/// Main container corner radius (rounded corners)
pub const CONTAINER_RADIUS: f32 = 12.0;
/// #4d4d4d full-opacity 2px inset border
pub const BORDER_WIDTH: f32 = 2.0;

// --- Selection indicator ---
pub const INDICATOR_WIDTH: f32 = 3.0;