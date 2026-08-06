use std::sync::atomic::Ordering;

use iced::keyboard::{key, Key, Modifiers};
use iced::time::Duration;
use iced::widget::{
    button, checkbox, column, container, mouse_area, row, scrollable, slider, text, text_input,
    Column,
};
use iced::{Color, Element, Length, Subscription, Theme};

use crate::config::Config;
use crate::debug_log;
use crate::orchestrator::{Orchestrator, Outcome, Request};
use crate::providers::SearchResult;
use crate::theme;
use crate::{
    EXIT_REQUESTED, HIDE_REQUESTED, HOTKEY_TRIGGERED, RESIZE_HEIGHT, RESIZE_REQUESTED, WINDOW_WIDTH,
};

const RESULTS_SCROLL_ID: &str = "results";
const SETTINGS_URL_ID: &str = "settings-url";

const SETTINGS_WINDOW_HEIGHT: f32 = 460.0;
const WIDTH_MIN: f32 = 400.0;
const WIDTH_MAX: f32 = 900.0;
const DEPTH_MIN: f32 = 4.0;
const DEPTH_MAX: f32 = 32.0;
const ENTRIES_MIN: f32 = 10_000.0;
const ENTRIES_MAX: f32 = 200_000.0;
const ENTRIES_STEP: f32 = 10_000.0;
const ACCENT_SWATCHES: [&str; 6] = [
    "#569cd4", "#e8540c", "#7cb86e", "#b56ec2", "#e06c75", "#4ec9b0",
];

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Search,
    Settings,
}

/// Editable settings state, initialized from the startup config and saved on exit.
#[derive(Debug, Clone)]
pub struct SettingsDraft {
    pub search_url: String,
    pub window_width: f32,
    pub accent: String,
    pub autostart: bool,
    pub file_index_depth: usize,
    pub file_index_entries: usize,
}

/// Build a settings draft from a config — used when opening the panel and
/// when resetting everything to factory defaults.
fn draft_from_config(cfg: &Config) -> SettingsDraft {
    SettingsDraft {
        search_url: cfg.search_url.clone(),
        window_width: cfg.window_width,
        accent: cfg.accent.clone(),
        autostart: cfg.autostart,
        file_index_depth: cfg.file_index_depth,
        file_index_entries: cfg.file_index_entries,
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    ResultClicked(usize),
    ResultRightClick(usize),
    Submit,
    KeyPressed(Key, Modifiers),
    Tick,
    SettingsBack,
    SearchUrlChanged(String),
    WidthChanged(f32),
    AccentChanged(String),
    AutostartChanged(bool),
    FileDepthChanged(f32),
    FileEntriesChanged(f32),
    ResetSettings,
}

pub struct ElementApp {
    pub engine: Orchestrator,
    pub input: String,
    pub results: Vec<SearchResult>,
    pub selected_index: i32,
    pub status: Option<String>,
    /// Contextual action hint for the selected result (files/clipboard).
    pub hint: Option<String>,
    pub search_revision: u64,
    pub mode: Mode,
    pub settings: SettingsDraft,
}

fn scroll_to_selected(selected_index: i32) -> iced::Task<Message> {
    if selected_index < 0 {
        return iced::Task::none();
    }
    let y = selected_index as f32 * theme::RESULT_HEIGHT;
    scrollable::scroll_to(
        scrollable::Id::new(RESULTS_SCROLL_ID),
        scrollable::AbsoluteOffset { x: 0.0, y },
    )
}

fn update_window_height(results: &[SearchResult], has_status: bool) {
    let h = adaptive_height(results, has_status);
    RESIZE_HEIGHT.store(h as u32, Ordering::Relaxed);
    RESIZE_REQUESTED.store(true, Ordering::SeqCst);
}

fn search(app: &mut ElementApp, query: &str) -> iced::Task<Message> {
    app.results = match app.engine.handle(Request::Search(query.to_string())) {
        Outcome::Results(results) => results,
        Outcome::Activated(_) | Outcome::Refreshed(_) | Outcome::Pinned(_) => Vec::new(),
    };
    app.selected_index = if app.results.is_empty() { -1 } else { 0 };
    app.search_revision = app.engine.revision();
    refresh_hint(app);
    update_window_height(&app.results, app.status.is_some() || app.hint.is_some());
    scroll_to_selected(app.selected_index)
}

/// Contextual hint for the selected result — shows available file actions or
/// the clipboard pin affordance. Cleared by a status message (status wins).
fn refresh_hint(app: &mut ElementApp) {
    app.hint = app
        .results
        .get(app.selected_index.max(0) as usize)
        .and_then(|r| match r.provider_id.as_str() {
            "files" => {
                Some("Enter open  ·  Alt+C copy path  ·  Alt+F copy file  ·  Alt+Enter reveal")
            }
            "clipboard" => Some("Enter copy  ·  Right-click to pin"),
            _ => None,
        })
        .map(str::to_string);
}

/// Toggle pin on a clipboard entry via the orchestrator, then refresh.
fn pin_clipboard(app: &mut ElementApp, index: usize) -> iced::Task<Message> {
    let Some(result) = app.results.get(index).cloned() else {
        return iced::Task::none();
    };
    match app
        .engine
        .handle(Request::PinClipboard(result.action.clone()))
    {
        Outcome::Pinned(pinned) => {
            app.status = Some(if pinned {
                "Pinned — stays in history".into()
            } else {
                "Unpinned".into()
            });
            app.hint = None;
            let query = app.input.clone();
            search(app, &query)
        }
        _ => iced::Task::none(),
    }
}

/// Run a secondary file action on the selected file result.
fn run_file_action(
    app: &mut ElementApp,
    action: crate::orchestrator::FileAction,
) -> iced::Task<Message> {
    let Some(result) = app.results.get(app.selected_index.max(0) as usize).cloned() else {
        return iced::Task::none();
    };
    if result.provider_id != "files" {
        return iced::Task::none();
    }
    match app.engine.handle(Request::FileAction {
        path: result.action,
        action,
    }) {
        Outcome::Activated(Ok(())) => {
            use crate::orchestrator::FileAction as A;
            app.status = Some(
                match action {
                    A::CopyPath => "Path copied to clipboard",
                    A::CopyFile => "File copied to clipboard",
                    A::Reveal => "Opened in Explorer",
                }
                .into(),
            );
        }
        Outcome::Activated(Err(error)) => {
            app.status = Some(format!("Could not: {error}"));
        }
        _ => {}
    }
    update_window_height(&app.results, true);
    iced::Task::none()
}

fn activate_result(app: &mut ElementApp, index: usize) -> iced::Task<Message> {
    let Some(result) = app.results.get(index).cloned() else {
        return iced::Task::none();
    };

    if result.kind == "settings" {
        open_settings(app);
        return iced::Task::none();
    }

    match app.engine.handle(Request::Activate(result.clone())) {
        Outcome::Activated(Ok(()))
            if matches!(result.kind.as_str(), "calc" | "emoji" | "clipboard") =>
        {
            app.status = Some("Copied to clipboard".into());
            app.hint = None;
            update_window_height(&app.results, true);
        }
        Outcome::Activated(Ok(())) => HIDE_REQUESTED.store(true, Ordering::SeqCst),
        Outcome::Activated(Err(error)) => {
            eprintln!("[element] failed to activate '{}': {error}", result.title);
            app.status = Some(format!("Could not open {}", result.title));
            app.hint = None;
            update_window_height(&app.results, true);
        }
        Outcome::Results(_) | Outcome::Refreshed(_) | Outcome::Pinned(_) => {}
    }

    iced::Task::none()
}

fn open_settings(app: &mut ElementApp) {
    debug_log!("UI: opening settings panel");
    app.mode = Mode::Settings;
    app.status = None;
    app.hint = None;
    app.settings = draft_from_config(&app.engine.config);
    RESIZE_HEIGHT.store(SETTINGS_WINDOW_HEIGHT as u32, Ordering::Relaxed);
    RESIZE_REQUESTED.store(true, Ordering::SeqCst);
}

/// Push the draft's file-index limits to the orchestrator so the files
/// provider re-indexes live (no restart needed).
fn apply_file_limits(app: &ElementApp) {
    app.engine.handle(Request::UpdateFileIndex {
        depth: app.settings.file_index_depth,
        entries: app.settings.file_index_entries,
    });
}

/// Persist the settings draft to `~/.element/config.toml`.
fn save_settings(app: &ElementApp) {
    let mut cfg = Config::load();
    cfg.search_url = app.settings.search_url.trim().to_string();
    cfg.window_width = app.settings.window_width;
    cfg.accent = app.settings.accent.clone();
    cfg.autostart = app.settings.autostart;
    cfg.file_index_depth = app.settings.file_index_depth;
    cfg.file_index_entries = app.settings.file_index_entries;
    cfg.save();
    debug_log!("UI: settings saved");
}

fn leave_settings(app: &mut ElementApp) -> iced::Task<Message> {
    save_settings(app);
    app.mode = Mode::Search;
    app.input.clear();
    app.status = None;
    app.hint = None;
    let search_task = search(app, "");
    iced::Task::batch(vec![text_input::focus("search"), search_task])
}

fn apply_width(app: &mut ElementApp, width: f32) {
    app.settings.window_width = width;
    WINDOW_WIDTH.store(width as u32, Ordering::Relaxed);
    RESIZE_HEIGHT.store(SETTINGS_WINDOW_HEIGHT as u32, Ordering::Relaxed);
    RESIZE_REQUESTED.store(true, Ordering::SeqCst);
}

fn apply_accent(app: &mut ElementApp, hex: &str) {
    app.settings.accent = hex.to_string();
    if let Some(color) = theme::parse_hex_color(hex) {
        theme::set_accent(color);
    }
}

pub fn update(app: &mut ElementApp, message: Message) -> iced::Task<Message> {
    match message {
        Message::SettingsBack => return leave_settings(app),
        Message::SearchUrlChanged(url) => {
            app.settings.search_url = url;
            return iced::Task::none();
        }
        Message::WidthChanged(width) => {
            apply_width(app, width);
            return iced::Task::none();
        }
        Message::AccentChanged(hex) => {
            apply_accent(app, &hex);
            return iced::Task::none();
        }
        Message::AutostartChanged(enabled) => {
            app.settings.autostart = enabled;
            crate::set_autostart(enabled);
            return iced::Task::none();
        }
        Message::FileDepthChanged(depth) => {
            app.settings.file_index_depth = depth as usize;
            apply_file_limits(app);
            return iced::Task::none();
        }
        Message::FileEntriesChanged(entries) => {
            app.settings.file_index_entries = entries as usize;
            apply_file_limits(app);
            return iced::Task::none();
        }
        Message::ResetSettings => {
            app.settings = draft_from_config(&Config::default());
            let accent = app.settings.accent.clone();
            let width = app.settings.window_width;
            apply_accent(app, &accent);
            apply_width(app, width);
            crate::set_autostart(app.settings.autostart);
            apply_file_limits(app);
            debug_log!("UI: settings reset to defaults");
            return iced::Task::none();
        }
        Message::InputChanged(text) => {
            app.input = text.clone();
            app.status = None;
            return search(app, &text);
        }
        Message::ResultClicked(index) => return activate_result(app, index),
        Message::ResultRightClick(index) => {
            let Some(result) = app.results.get(index).cloned() else {
                return iced::Task::none();
            };
            if result.provider_id == "clipboard" {
                return pin_clipboard(app, index);
            }
            if result.provider_id == "files" {
                match app.engine.handle(Request::FileAction {
                    path: result.action,
                    action: crate::orchestrator::FileAction::CopyPath,
                }) {
                    Outcome::Activated(Ok(())) => {
                        app.status = Some("Path copied to clipboard".into())
                    }
                    Outcome::Activated(Err(error)) => {
                        app.status = Some(format!("Could not: {error}"))
                    }
                    _ => {}
                }
                app.hint = None;
                update_window_height(&app.results, true);
            }
            return iced::Task::none();
        }
        Message::Submit => {
            if app.mode == Mode::Search && app.selected_index >= 0 {
                return activate_result(app, app.selected_index as usize);
            }
        }
        Message::KeyPressed(key, mods) => {
            if app.mode == Mode::Settings {
                if key == Key::Named(key::Named::Escape) {
                    debug_log!("UI: Escape in settings – back to search");
                    return leave_settings(app);
                }
                // Backspace with an empty URL field goes back to search;
                // while the field has text it stays an edit key.
                if key == Key::Named(key::Named::Backspace) && app.settings.search_url.is_empty() {
                    debug_log!("UI: Backspace in settings – back to search");
                    return leave_settings(app);
                }
                return iced::Task::none();
            }
            let ctrl = mods.control();
            let move_selection = |app: &mut ElementApp, delta: i32| -> iced::Task<Message> {
                let count = app.results.len() as i32;
                if count > 0 {
                    app.selected_index = if delta > 0 {
                        if app.selected_index >= count - 1 {
                            0
                        } else {
                            app.selected_index + 1
                        }
                    } else if app.selected_index <= 0 {
                        count - 1
                    } else {
                        app.selected_index - 1
                    };
                }
                refresh_hint(app);
                scroll_to_selected(app.selected_index)
            };
            match key {
                Key::Named(key::Named::Escape) => {
                    // Single Esc hides (same as Alt+Space close) — does not quit.
                    debug_log!("UI: Escape – hiding launcher");
                    app.input.clear();
                    app.status = None;
                    app.hint = None;
                    HIDE_REQUESTED.store(true, Ordering::SeqCst);
                }
                Key::Named(key::Named::ArrowUp) => return move_selection(app, -1),
                Key::Named(key::Named::ArrowDown) => return move_selection(app, 1),
                Key::Character(character) if ctrl && (character == "p") => {
                    return move_selection(app, -1);
                }
                Key::Character(character) if ctrl && (character == "n") => {
                    return move_selection(app, 1);
                }
                // File result actions — Alt combos avoid the text input's own
                // Ctrl+C/X/V handling.
                Key::Named(key::Named::Enter) if mods.alt() => {
                    return run_file_action(app, crate::orchestrator::FileAction::Reveal);
                }
                Key::Character(character) if mods.alt() && character == "c" => {
                    return run_file_action(app, crate::orchestrator::FileAction::CopyPath);
                }
                Key::Character(character) if mods.alt() && character == "f" => {
                    return run_file_action(app, crate::orchestrator::FileAction::CopyFile);
                }
                Key::Character(character) if ctrl && !mods.shift() => {
                    if let Ok(number) = character.parse::<usize>() {
                        if (1..=9).contains(&number) && number <= app.results.len() {
                            app.selected_index = number as i32 - 1;
                            refresh_hint(app);
                            return scroll_to_selected(app.selected_index);
                        }
                    }
                }
                _ => {}
            }
        }
        Message::Tick => {
            if EXIT_REQUESTED.swap(false, Ordering::SeqCst) {
                debug_log!("UI: EXIT_REQUESTED – exiting Iced application");
                return iced::exit();
            }
            if HOTKEY_TRIGGERED.swap(false, Ordering::SeqCst) {
                debug_log!("UI: HOTKEY_TRIGGERED received – refreshing and focusing input");
                if let Outcome::Refreshed(rev) = app.engine.handle(Request::Refresh) {
                    app.search_revision = rev;
                }
                app.input.clear();
                app.status = None;
                app.hint = None;
                let search_task = search(app, "");
                return iced::Task::batch(vec![text_input::focus("search"), search_task]);
            }
            if app.engine.revision() != app.search_revision {
                let query = app.input.clone();
                return search(app, &query);
            }
        }
    }
    iced::Task::none()
}

pub fn view(app: &ElementApp) -> Element<'_, Message> {
    if app.mode == Mode::Settings {
        return settings_view(app);
    }

    let search = text_input::TextInput::new("Search apps or type a web query...", &app.input)
        .id("search")
        .on_input(Message::InputChanged)
        .on_submit(Message::Submit)
        .padding([theme::INPUT_PADDING_VERTICAL, theme::INPUT_PADDING_SIDES])
        .style(element_input_style);

    let header = row![search]
        .spacing(0)
        .padding([theme::HEADER_PADDING_VERTICAL, theme::CONTENT_PADDING_SIDES])
        .align_y(iced::Alignment::Center)
        .height(theme::SEARCH_BAR_HEIGHT);

    let mut list = Column::new().spacing(0);

    for (i, result) in app.results.iter().enumerate() {
        let selected = i == app.selected_index as usize;
        list = list.push(result_row(i, result, selected));
    }

    let scroll = iced::widget::Scrollable::new(list)
        .id(scrollable::Id::new(RESULTS_SCROLL_ID))
        .height(Length::Shrink)
        .width(Length::Fill)
        .style(element_scrollable_style);

    let mut content = Column::new().push(header);
    if let Some(status) = &app.status {
        content = content.push(
            container(
                text(status)
                    .color(theme::TEXT_ERROR)
                    .size(theme::SUBTITLE_SIZE),
            )
            .padding([theme::SPACING_SM, theme::CONTENT_PADDING_SIDES])
            .height(theme::STATUS_HEIGHT),
        );
    } else if let Some(hint) = &app.hint {
        content = content.push(
            container(
                text(hint)
                    .color(theme::TEXT_MUTED)
                    .size(theme::SUBTITLE_SIZE),
            )
            .padding([theme::SPACING_SM, theme::CONTENT_PADDING_SIDES])
            .height(theme::STATUS_HEIGHT),
        );
    }
    let content = content.push(scroll).width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Shrink)
        .style(element_container_style)
        .into()
}

pub fn subscription(_app: &ElementApp) -> Subscription<Message> {
    Subscription::batch(vec![
        iced::keyboard::on_key_press(|key, mods| Some(Message::KeyPressed(key, mods))),
        iced::time::every(Duration::from_millis(30)).map(|_| Message::Tick),
    ])
}

fn result_row(index: usize, result: &SearchResult, selected: bool) -> Element<'_, Message> {
    let icon: Element<'_, Message> = if let Some((ref pixels, w, h)) = result.icon_rgba {
        let handle = iced::widget::image::Handle::from_rgba(w, h, pixels.clone());
        iced::widget::image(handle)
            .width(Length::Shrink)
            .height(theme::ICON_SIZE)
            .into()
    } else {
        container(text("").width(theme::ICON_SIZE))
            .width(theme::ICON_SIZE)
            .height(theme::ICON_SIZE)
            .into()
    };

    let title = text(&result.title)
        .color(theme::TEXT_PRIMARY)
        .size(theme::TITLE_SIZE);

    let subtitle = text(&result.subtitle)
        .color(theme::TEXT_MUTED)
        .size(theme::SUBTITLE_SIZE);

    let indicator = if selected {
        container(text("").width(theme::INDICATOR_WIDTH))
            .style(|_: &Theme| iced::widget::container::Style {
                background: Some(theme::accent().into()),
                ..Default::default()
            })
            .width(theme::INDICATOR_WIDTH)
    } else {
        container(text("").width(theme::INDICATOR_WIDTH)).width(theme::INDICATOR_WIDTH)
    };

    let item = row![
        indicator,
        icon,
        column![title, subtitle]
            .spacing(theme::SPACING_SM)
            .width(Length::Fill),
    ]
    .spacing(theme::SPACING_MD)
    .padding([0.0, theme::CONTENT_PADDING_SIDES])
    .height(theme::RESULT_HEIGHT)
    .align_y(iced::Alignment::Center);

    let bg = if selected {
        theme::BG_SELECTED
    } else {
        Color::TRANSPARENT
    };
    let item = container(item)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(bg.into()),
            ..Default::default()
        })
        .width(Length::Fill);

    mouse_area(item)
        .on_press(Message::ResultClicked(index))
        .on_right_press(Message::ResultRightClick(index))
        .into()
}

fn settings_row<'a>(title: &'a str, content: Element<'a, Message>) -> Element<'a, Message> {
    row![
        text(title)
            .color(theme::TEXT_MUTED)
            .size(theme::TITLE_SIZE)
            .width(Length::FillPortion(2)),
        container(content).width(Length::FillPortion(3)),
    ]
    .spacing(theme::SPACING_MD)
    .padding([6.0, theme::CONTENT_PADDING_SIDES])
    .align_y(iced::Alignment::Center)
    .into()
}

fn settings_view(app: &ElementApp) -> Element<'_, Message> {
    let title = text("Settings")
        .color(theme::TEXT_PRIMARY)
        .size(theme::TITLE_SIZE)
        .width(Length::Fill);

    let back = button(text("← Back"))
        .on_press(Message::SettingsBack)
        .style(accent_button_style)
        .padding([4.0, 10.0]);

    let header = row![title, back]
        .padding([8.0, theme::CONTENT_PADDING_SIDES])
        .align_y(iced::Alignment::Center);

    let width_value = text(format!("{} px", app.settings.window_width.round() as u32))
        .color(theme::TEXT_PRIMARY)
        .size(theme::SUBTITLE_SIZE)
        .width(Length::FillPortion(1));
    let width = row![
        slider(
            WIDTH_MIN..=WIDTH_MAX,
            app.settings.window_width,
            Message::WidthChanged
        )
        .width(Length::FillPortion(3)),
        width_value,
    ]
    .spacing(theme::SPACING_MD)
    .align_y(iced::Alignment::Center);
    let width_row = settings_row("Window width", width.into());

    let url_input = text_input::TextInput::new(
        "https://duckduckgo.com/search?q=%s",
        &app.settings.search_url,
    )
    .id(SETTINGS_URL_ID)
    .on_input(Message::SearchUrlChanged)
    .padding([6.0, 8.0])
    .style(element_input_style);
    let url_row = settings_row("Search URL", url_input.into());

    let swatches = row(ACCENT_SWATCHES
        .iter()
        .map(|hex| {
            let color = theme::parse_hex_color(hex).unwrap_or(theme::accent());
            let selected = app.settings.accent.eq_ignore_ascii_case(hex);
            let swatch = container(text(""))
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(color.into()),
                    border: iced::Border {
                        radius: 4.0.into(),
                        width: if selected { 2.0 } else { 0.0 },
                        color: if selected {
                            theme::TEXT_PRIMARY
                        } else {
                            Color::TRANSPARENT
                        },
                    },
                    ..Default::default()
                })
                .width(22.0)
                .height(22.0);
            button(swatch)
                .on_press(Message::AccentChanged((*hex).to_string()))
                .padding(0)
                .style(|_: &Theme, _: button::Status| iced::widget::button::Style {
                    background: None,
                    border: Default::default(),
                    text_color: Color::TRANSPARENT,
                    ..Default::default()
                })
                .into()
        })
        .collect::<Vec<Element<'static, Message>>>())
    .spacing(theme::SPACING_MD)
    .align_y(iced::Alignment::Center);
    let accent_row = settings_row("Accent", swatches.into());

    let autostart = checkbox("Run Element at startup", app.settings.autostart)
        .on_toggle(Message::AutostartChanged)
        .text_size(theme::TITLE_SIZE);
    let autostart_row = settings_row("Startup", autostart.into());

    let depth_value = text(format!("{} levels", app.settings.file_index_depth))
        .color(theme::TEXT_PRIMARY)
        .size(theme::SUBTITLE_SIZE)
        .width(Length::FillPortion(1));
    let depth = row![
        slider(
            DEPTH_MIN..=DEPTH_MAX,
            app.settings.file_index_depth as f32,
            Message::FileDepthChanged
        )
        .step(1.0_f32)
        .width(Length::FillPortion(3)),
        depth_value,
    ]
    .spacing(theme::SPACING_MD)
    .align_y(iced::Alignment::Center);
    let depth_row = settings_row("File index depth", depth.into());

    let entries_value = text(format!("{}", app.settings.file_index_entries))
        .color(theme::TEXT_PRIMARY)
        .size(theme::SUBTITLE_SIZE)
        .width(Length::FillPortion(1));
    let entries = row![
        slider(
            ENTRIES_MIN..=ENTRIES_MAX,
            app.settings.file_index_entries as f32,
            Message::FileEntriesChanged
        )
        .step(ENTRIES_STEP)
        .width(Length::FillPortion(3)),
        entries_value,
    ]
    .spacing(theme::SPACING_MD)
    .align_y(iced::Alignment::Center);
    let entries_row = settings_row("File index entries", entries.into());

    let hotkey = text(format!(
        "{}  ·  edit in config.toml",
        app.engine.config.hotkey
    ))
    .color(theme::TEXT_PRIMARY)
    .size(theme::TITLE_SIZE);
    let hotkey_row = settings_row("Hotkey", hotkey.into());

    let hint = text("Changes apply live and are saved automatically.")
        .color(theme::TEXT_MUTED)
        .size(theme::SUBTITLE_SIZE);

    let reset = button(text("Reset to defaults"))
        .on_press(Message::ResetSettings)
        .style(accent_button_style)
        .padding([4.0, 10.0]);

    let footer = row![container(hint).width(Length::Fill), reset]
        .padding([8.0, theme::CONTENT_PADDING_SIDES])
        .align_y(iced::Alignment::Center);

    let list = column![
        header,
        width_row,
        url_row,
        accent_row,
        autostart_row,
        depth_row,
        entries_row,
        hotkey_row,
        footer,
    ]
    .spacing(theme::SPACING_SM)
    .width(Length::Fill);

    container(list)
        .width(Length::Fill)
        .height(Length::Shrink)
        .style(element_container_style)
        .into()
}

fn adaptive_height(results: &[SearchResult], has_status: bool) -> f32 {
    let count = (results.len().min(theme::MAX_VISIBLE_RESULTS)) as f32;
    let status_height = if has_status {
        theme::STATUS_HEIGHT
    } else {
        0.0
    };
    let h = theme::SEARCH_BAR_HEIGHT
        + status_height
        + count * theme::RESULT_HEIGHT
        + theme::BOTTOM_PADDING;
    h.clamp(theme::MIN_WINDOW_HEIGHT, theme::MAX_WINDOW_HEIGHT)
}

fn element_input_style(_theme: &Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: theme::BG_INPUT.into(),
        border: iced::Border {
            radius: theme::BORDER_RADIUS.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        icon: theme::TEXT_ICON,
        placeholder: theme::TEXT_PLACEHOLDER,
        value: theme::TEXT_PRIMARY,
        selection: theme::accent(),
    }
}

fn element_scrollable_style(_theme: &Theme, _status: scrollable::Status) -> scrollable::Style {
    scrollable::Style {
        container: Default::default(),
        vertical_rail: scrollable::Rail {
            background: None,
            border: Default::default(),
            scroller: scrollable::Scroller {
                color: Color::TRANSPARENT,
                border: Default::default(),
            },
        },
        horizontal_rail: scrollable::Rail {
            background: None,
            border: Default::default(),
            scroller: scrollable::Scroller {
                color: Color::TRANSPARENT,
                border: Default::default(),
            },
        },
        gap: None,
    }
}

fn element_container_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(theme::BG_PRIMARY.into()),
        border: iced::Border {
            radius: theme::CONTAINER_RADIUS.into(),
            width: theme::BORDER_WIDTH,
            color: theme::BORDER,
        },
        ..Default::default()
    }
}

fn accent_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => theme::BG_SELECTED,
        _ => theme::BG_INPUT,
    };
    button::Style {
        background: Some(bg.into()),
        border: iced::Border {
            radius: theme::BORDER_RADIUS.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        text_color: theme::TEXT_PRIMARY,
        ..Default::default()
    }
}
