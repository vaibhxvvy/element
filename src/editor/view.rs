use gpui::{Context, Render, Window, div, px, rgb};
use gpui::prelude::*;

use super::buffer::TextBuffer;
use super::find::FindState;

pub struct EditorView {
    pub buffer: TextBuffer,
    pub find: FindState,
}

impl EditorView {
    pub fn new(buffer: TextBuffer) -> Self {
        Self {
            buffer,
            find: FindState::default(),
        }
    }
}

impl Render for EditorView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.buffer.text.clone();
        let lines: Vec<&str> = if text.is_empty() {
            vec![""]
        } else {
            text.lines().collect()
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .p(px(12.0))
            .children(lines.iter().enumerate().map(|(i, line)| {
                let line_num = i + 1;
                div()
                    .flex()
                    .child(
                        div()
                            .w(px(40.0))
                            .text_color(rgb(0x4D2AC4))
                            .child(format!("{:>4}", line_num)),
                    )
                    .child(div().flex().child(if line.is_empty() {
                        "\u{00A0}".to_string()
                    } else {
                        line.to_string()
                    }))
            }))
    }
}
