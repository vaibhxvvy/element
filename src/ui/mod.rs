use std::sync::atomic::Ordering;

use iced::{
    widget::{column, container, mouse_area, row, text, text_input, Column, TextInput},
    Color, Element, Length, Subscription, Theme,
};
use iced::keyboard::{key, Key, Modifiers};
use iced::time::Duration;

use crate::app::SearchResult;
use crate::theme;
use crate::{HIDE_REQUESTED, HOTKEY_TRIGGERED, RESIZE_HEIGHT, RESIZE_REQUESTED};

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    KeyPressed(Key, Modifiers),
    Tick,
}

pub struct ElementApp {
    pub engine: crate::app::SearchEngine,
    pub input: String,
    pub results: Vec<SearchResult>,
    pub selected_index: i32,
}

pub fn update(app: &mut ElementApp, message: Message) -> iced::Task<Message> {
    match message {
        Message::InputChanged(text) => {
            app.input = text.clone();
            app.results = app.engine.search(&text);
            app.selected_index = if text.is_empty() { -1 } else { 0 };
            let h = adaptive_height(&app.results);
            RESIZE_HEIGHT.store(h as u32, Ordering::Relaxed);
            RESIZE_REQUESTED.store(true, Ordering::SeqCst);
        }
        Message::KeyPressed(key, _mods) => match key {
            Key::Named(key::Named::Escape) => {
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
            }
            Key::Named(key::Named::Enter) => {
                if app.selected_index >= 0 {
                    let idx = app.selected_index as usize;
                    if idx < app.results.len() {
                        let item = app.results[idx].clone();
                        let _ = app.engine.activate(&item);
                    }
                }
                HIDE_REQUESTED.store(true, Ordering::SeqCst);
            }
            _ => {}
        },
        Message::Tick => {
            if HOTKEY_TRIGGERED.swap(false, Ordering::SeqCst) {
                app.engine.refresh_all();
                app.input.clear();
                app.results.clear();
                app.selected_index = -1;
                let h = adaptive_height(&[]);
                RESIZE_HEIGHT.store(h as u32, Ordering::Relaxed);
                RESIZE_REQUESTED.store(true, Ordering::SeqCst);
                return text_input::focus("search");
            }
        }
    }
    iced::Task::none()
}

pub fn view(app: &ElementApp) -> Element<'_, Message> {
    let search = TextInput::new("Search apps, files, or type anything...", &app.input)
        .id("search")
        .on_input(Message::InputChanged)
        .padding([theme::INPUT_PADDING_TOP, theme::INPUT_PADDING_SIDES])
        .style(element_input_style);

    let mut list = Column::new().spacing(0);

    for (i, result) in app.results.iter().enumerate() {
        let selected = i == app.selected_index as usize;
        list = list.push(result_row(i, result, selected));
    }

    let scroll = iced::widget::Scrollable::new(list)
        .height(Length::Shrink)
        .width(Length::Fill);

    let content = Column::new()
        .push(search)
        .push(scroll)
        .width(Length::Fill);

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

fn result_row(_i: usize, result: &SearchResult, selected: bool) -> Element<'_, Message> {
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
            .style(|_: &Theme| {
                iced::widget::container::Style {
                    background: Some(theme::ACCENT.into()),
                    ..Default::default()
                }
            })
            .width(theme::INDICATOR_WIDTH)
    } else {
        container(text("").width(theme::INDICATOR_WIDTH))
            .width(theme::INDICATOR_WIDTH)
    };

    let item = row![
        indicator,
        icon,
        column![title, subtitle].spacing(theme::SPACING_SM).width(Length::Fill),
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

    mouse_area(item).into()
}

fn adaptive_height(results: &[SearchResult]) -> f32 {
    let count = (results.len().min(theme::MAX_VISIBLE_RESULTS)) as f32;
    let h = theme::SEARCH_BAR_HEIGHT + count * theme::RESULT_HEIGHT + theme::BOTTOM_PADDING;
    h.min(theme::MAX_WINDOW_HEIGHT).max(theme::MIN_WINDOW_HEIGHT)
}

fn element_input_style(
    _theme: &Theme,
    _status: text_input::Status,
) -> text_input::Style {
    text_input::Style {
        background: theme::BG_INPUT.into(),
        border: iced::Border {
            radius: theme::BORDER_RADIUS.into(),
            width: theme::BORDER_WIDTH,
            color: Color::TRANSPARENT,
        },
        icon: theme::TEXT_ICON,
        placeholder: theme::TEXT_PLACEHOLDER,
        value: theme::TEXT_PRIMARY,
        selection: theme::ACCENT,
    }
}

fn element_container_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(theme::BG_PRIMARY.into()),
        ..Default::default()
    }
}
