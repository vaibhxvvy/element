use std::sync::atomic::Ordering;

use iced::keyboard::{key, Key, Modifiers};
use iced::time::Duration;
use iced::{
    widget::{column, container, mouse_area, row, scrollable, text, text_input, Column},
    Color, Element, Length, Subscription, Theme,
};

use crate::debug_log;
use crate::orchestrator::{Orchestrator, Outcome, Request};
use crate::providers::SearchResult;
use crate::theme;
use crate::{EXIT_REQUESTED, HIDE_REQUESTED, HOTKEY_TRIGGERED, RESIZE_HEIGHT, RESIZE_REQUESTED};

const RESULTS_SCROLL_ID: &str = "results";

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    ResultClicked(usize),
    Submit,
    KeyPressed(Key, Modifiers),
    Tick,
}

pub struct ElementApp {
    pub engine: Orchestrator,
    pub input: String,
    pub results: Vec<SearchResult>,
    pub selected_index: i32,
    pub status: Option<String>,
    pub search_revision: u64,
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
        Outcome::Activated(_) | Outcome::Refreshed(_) => Vec::new(),
    };
    app.selected_index = if app.results.is_empty() { -1 } else { 0 };
    app.search_revision = app.engine.revision();
    update_window_height(&app.results, app.status.is_some());
    scroll_to_selected(app.selected_index)
}

fn activate_result(app: &mut ElementApp, index: usize) -> iced::Task<Message> {
    let Some(result) = app.results.get(index).cloned() else {
        return iced::Task::none();
    };

    match app.engine.handle(Request::Activate(result.clone())) {
        Outcome::Activated(Ok(()))
            if matches!(result.kind.as_str(), "calc" | "emoji" | "clipboard") =>
        {
            app.status = Some("Copied to clipboard".into());
            update_window_height(&app.results, true);
        }
        Outcome::Activated(Ok(())) => HIDE_REQUESTED.store(true, Ordering::SeqCst),
        Outcome::Activated(Err(error)) => {
            eprintln!("[element] failed to activate '{}': {error}", result.title);
            app.status = Some(format!("Could not open {}", result.title));
            update_window_height(&app.results, true);
        }
        Outcome::Results(_) | Outcome::Refreshed(_) => {}
    }

    iced::Task::none()
}

pub fn update(app: &mut ElementApp, message: Message) -> iced::Task<Message> {
    match message {
        Message::InputChanged(text) => {
            app.input = text.clone();
            app.status = None;
            return search(app, &text);
        }
        Message::ResultClicked(index) => return activate_result(app, index),
        Message::Submit => {
            if app.selected_index >= 0 {
                return activate_result(app, app.selected_index as usize);
            }
        }
        Message::KeyPressed(key, _mods) => match key {
            Key::Named(key::Named::Escape) => {
                // Single Esc hides (same as Alt+Space close) — does not quit.
                debug_log!("UI: Escape – hiding launcher");
                app.input.clear();
                app.status = None;
                HIDE_REQUESTED.store(true, Ordering::SeqCst);
            }
            Key::Named(key::Named::ArrowUp) => {
                let count = app.results.len() as i32;
                if count > 0 {
                    app.selected_index = if app.selected_index <= 0 {
                        count - 1
                    } else {
                        app.selected_index - 1
                    };
                }
                return scroll_to_selected(app.selected_index);
            }
            Key::Named(key::Named::ArrowDown) => {
                let count = app.results.len() as i32;
                if count > 0 {
                    app.selected_index = if app.selected_index >= count - 1 {
                        0
                    } else {
                        app.selected_index + 1
                    };
                }
                return scroll_to_selected(app.selected_index);
            }
            _ => {}
        },
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
                background: Some(theme::ACCENT.into()),
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
        selection: theme::ACCENT,
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
