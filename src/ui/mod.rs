use std::sync::atomic::Ordering;

use iced::{
    widget::{column, container, mouse_area, row, text, text_input, Column, TextInput},
    Color, Element, Length, Subscription, Theme,
};
use iced::keyboard::{key, Key, Modifiers};
use iced::time::Duration;

use crate::app::{SearchEngine, SearchResult};
use crate::{HIDE_REQUESTED, HOTKEY_TRIGGERED};

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    KeyPressed(Key, Modifiers),
    Tick,
}

pub struct ElementApp {
    pub engine: SearchEngine,
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
                        let item = &app.results[idx];
                        app.engine.activate(&item.kind, &item.title, &app.input);
                    }
                }
                HIDE_REQUESTED.store(true, Ordering::SeqCst);
            }
            _ => {}
        },
        Message::Tick => {
            if HOTKEY_TRIGGERED.swap(false, Ordering::SeqCst) {
                app.engine.refresh_apps();
                app.input.clear();
                app.results.clear();
                app.selected_index = -1;
            }
        }
    }
    iced::Task::none()
}

pub fn view(app: &ElementApp) -> Element<'_, Message> {
    let search = TextInput::new("Search apps, files, or type anything...", &app.input)
        .on_input(Message::InputChanged)
        .padding([14, 16])
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
    let title = text(&result.title)
        .color(Color::from_rgb(30.0 / 255.0, 30.0 / 255.0, 35.0 / 255.0))
        .size(13);

    let subtitle = text(&result.subtitle)
        .color(Color::from_rgb(100.0 / 255.0, 100.0 / 255.0, 110.0 / 255.0))
        .size(11);

    let indicator = if selected {
        container(text("").width(3))
            .style(|_: &Theme| {
                iced::widget::container::Style {
                    background: Some(
                        Color::from_rgb(150.0 / 255.0, 150.0 / 255.0, 255.0 / 255.0).into(),
                    ),
                    ..Default::default()
                }
            })
            .width(3)
    } else {
        container(text("").width(3)).width(3)
    };

    let item = row![
        indicator,
        column![title, subtitle].spacing(1).width(Length::Fill),
    ]
    .spacing(12)
    .padding([0, 16])
    .height(42)
    .align_y(iced::Alignment::Center);

    let bg = if selected {
        Color::from_rgb(235.0 / 255.0, 235.0 / 255.0, 245.0 / 255.0)
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

fn element_input_style(
    theme: &Theme,
    _status: text_input::Status,
) -> text_input::Style {
    text_input::Style {
        background: Color::from_rgb(245.0 / 255.0, 245.0 / 255.0, 245.0 / 255.0).into(),
        border: iced::Border {
            radius: 0.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        icon: Color::from_rgb(120.0 / 255.0, 120.0 / 255.0, 128.0 / 255.0),
        placeholder: Color::from_rgb(160.0 / 255.0, 160.0 / 255.0, 168.0 / 255.0),
        value: Color::from_rgb(30.0 / 255.0, 30.0 / 255.0, 35.0 / 255.0),
        selection: theme.extended_palette().primary.strong.color,
    }
}

fn element_container_style(_theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Color::from_rgb(255.0, 255.0, 255.0).into()),
        ..Default::default()
    }
}
