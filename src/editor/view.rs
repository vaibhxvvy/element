use gpui::*;
use gpui::prelude::*;

use super::buffer::TextBuffer;
use super::find::FindState;

actions!(editor, [
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    NewFile,
    OpenFile,
    Save,
    SaveAs,
    Find,
    FindNext,
    InsertDateTime,
    Quit,
]);

pub struct EditorView {
    pub focus_handle: FocusHandle,
    pub buffer: TextBuffer,
    pub find: FindState,
    pub show_find: bool,
    pub show_save_dialog: bool,
    error_msg: String,
}

impl EditorView {
    pub fn new(buffer: TextBuffer, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            buffer,
            find: FindState::default(),
            show_find: false,
            show_save_dialog: false,
            error_msg: String::new(),
        }
    }
}

impl Focusable for EditorView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for EditorView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(window);
        let focus = self.focus_handle.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .key_context("EditorView")
            .track_focus(&focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let ks = &event.keystroke;
                let has_ctrl = ks.modifiers.control || ks.modifiers.platform;
                let has_shift = ks.modifiers.shift;

                if has_ctrl {
                    match ks.key.as_str() {
                        "n" => this.do_new_file(window, cx),
                        "o" => this.do_open_file(window, cx),
                        "s" if has_shift => this.do_save_as(window, cx),
                        "s" => this.do_save(window, cx),
                        "f" => this.do_toggle_find(window, cx),
                        "d" => this.do_insert_date_time(window, cx),
                        "q" => this.do_quit(window, cx),
                        _ => {}
                    }
                    return;
                }
                match ks.key.as_str() {
                    "f3" => this.do_find_next(window, cx),
                    _ => {}
                }
                match ks.key.as_str() {
                    "backspace" => { this.buffer.delete_backward(); }
                    "delete" => { this.buffer.delete_forward(); }
                    "left" => { this.buffer.cursor_left(); }
                    "right" => { this.buffer.cursor_right(); }
                    "up" => { this.buffer.cursor_up(); }
                    "down" => { this.buffer.cursor_down(); }
                    "home" => { this.buffer.home(); }
                    "end" => { this.buffer.end(); }
                    "enter" => { this.buffer.insert_newline(); }
                    "tab" => { this.buffer.insert_str("    "); }
                    _ => {
                        if let Some(ch) = &ks.key_char {
                            for c in ch.chars() {
                                if !c.is_control() {
                                    this.buffer.insert_char(c);
                                }
                            }
                        }
                    }
                }
            }))
            .child(self.render_views(window, cx, is_focused))
    }
}

impl EditorView {
    fn render_views(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        is_focused: bool,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.render_find_bar(cx))
            .child(self.render_text_area(is_focused))
            .child(self.render_status_bar())
            .when(self.show_save_dialog, |d| d.child(self.render_save_dialog(cx)))
    }

    fn render_find_bar(&self, _cx: &mut Context<Self>) -> Div {
        if !self.show_find {
            return div();
        }

        div()
            .flex()
            .h(px(28.0))
            .bg(rgb(0x12101A))
            .border_b_1()
            .border_color(rgb(0x2A2740))
            .px(px(8.0))
            .items_center()
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .items_center()
                    .child(div().text_color(rgb(0xA7A0BD)).text_size(px(12.0)).child("Find:"))
                    .child(
                        div()
                            .flex()
                            .w(px(200.0))
                            .h(px(22.0))
                            .bg(rgb(0x1C1A2A))
                            .rounded(px(4.0))
                            .px(px(6.0))
                            .child(
                                div()
                                    .text_color(rgb(0xFDFAFE))
                                    .text_size(px(12.0))
                                    .child(self.find.query.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_color(rgb(0xA7A0BD))
                            .text_size(px(12.0))
                            .child(
                                (self.find.match_count > 0)
                                    .then(|| format!("({})", self.find.match_count))
                                    .unwrap_or_default(),
                            ),
                    )
                    .child(
                        div()
                            .text_color(rgb(0x6D4AFA))
                            .text_size(px(12.0))
                            .cursor_pointer()
                            .child("Close")
                            .on_mouse_up(MouseButton::Left, _cx.listener(|this, _: &MouseUpEvent, _w, _cx| {
                                this.show_find = false;
                                this.find.reset();
                            })),
                    ),
            )
    }

    fn render_text_area(&self, is_focused: bool) -> impl IntoElement {
        let text = &self.buffer.text;
        let lines: Vec<&str> = if text.is_empty() {
            vec![""]
        } else {
            text.lines().collect()
        };

        let cursor_line = self.buffer.cursor_line;
        let cursor_col = self.buffer.cursor_col;

        div()
            .flex()
            .flex_col()
            .size_full()
            .id("editor-area")
            .overflow_y_scroll()
            .p(px(12.0))
            .bg(rgb(0x08060D))
            .children(lines.iter().enumerate().map(move |(i, line)| {
                let line_num = i + 1;
                let is_cl = line_num == cursor_line;

                div()
                    .flex()
                    .h(px(20.0))
                    .child(
                        div()
                            .w(px(40.0))
                            .text_color(rgb(0x4D2AC4))
                            .child(format!("{:>4}", line_num)),
                    )
                    .child(if is_cl {
                        let cursor_byte = line
                            .char_indices()
                            .nth(cursor_col.saturating_sub(1))
                            .map(|(i, _)| i)
                            .unwrap_or(line.len());

                        if cursor_col == 1 {
                            div()
                                .flex()
                                .child(
                                    div()
                                        .w(px(2.0))
                                        .h(px(16.0))
                                        .bg(if is_focused { rgb(0xFDFAFE) } else { rgb(0x4D2AC4) }),
                                )
                                .child(line.to_string())
                        } else {
                            div()
                                .flex()
                                .child(line[..cursor_byte].to_string())
                                .child(
                                    div()
                                        .w(px(2.0))
                                        .h(px(16.0))
                                        .bg(if is_focused { rgb(0xFDFAFE) } else { rgb(0x4D2AC4) }),
                                )
                                .child(line[cursor_byte..].to_string())
                        }
                    } else {
                        div().child(line.to_string())
                    })
            }))
    }

    fn render_status_bar(&self) -> Div {
        let modified = if self.buffer.is_dirty() { " (modified)" } else { "" };
        let status = format!("Ln {}, Col {}{}",
            self.buffer.cursor_line, self.buffer.cursor_col, modified);

        div()
            .flex()
            .h(px(24.0))
            .bg(rgb(0x12101A))
            .border_t_1()
            .border_color(rgb(0x2A2740))
            .px(px(12.0))
            .items_center()
            .child(
                div()
                    .flex()
                    .w_full()
                    .child(div().text_color(rgb(0xA7A0BD)).text_size(px(11.0)).child(status))
                    .child(
                        div()
                            .flex()
                            .ml_auto()
                            .text_color(rgb(0xA7A0BD))
                            .text_size(px(11.0))
                            .child(self.buffer.file_name()),
                    ),
            )
    }

    fn render_save_dialog(&self, _cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .size_full()
            .justify_center()
            .items_center()
            .bg(hsla(0.0, 0.0, 0.0, 0.6))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .p(px(24.0))
                    .bg(rgb(0x1C1A2A))
                    .rounded(px(8.0))
                    .child(div().text_color(rgb(0xFDFAFE)).child("Do you want to save changes?"))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .child(save_dialog_button("Save", rgb(0x6D4AFA)))
                            .child(save_dialog_button("Don't Save", rgb(0x2A2740)))
                            .child(save_dialog_button("Cancel", rgb(0x2A2740))),
                    ),
            )
    }

    fn do_new_file(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if self.buffer.is_dirty() {
            self.show_save_dialog = true;
            return;
        }
        self.buffer.new_file();
        self.show_find = false;
        self.show_save_dialog = false;
    }

    fn do_open_file(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if self.buffer.is_dirty() {
            return;
        }
        if let Some(path) = tinyfiledialogs::open_file_dialog("Open File", "", None) {
            let path = std::path::PathBuf::from(&path);
            self.buffer = TextBuffer::from_path(&path);
            self.show_find = false;
        }
    }

    fn do_save(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if self.buffer.file_path.is_some() {
            if let Err(e) = self.buffer.save() {
                self.error_msg = e;
            }
        } else {
            self.do_save_as(_window, _cx);
        }
    }

    fn do_save_as(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(path) = tinyfiledialogs::save_file_dialog("Save As", "") {
            let path = std::path::PathBuf::from(&path);
            if let Err(e) = self.buffer.save_as(path) {
                self.error_msg = e;
            }
        }
    }

    fn do_toggle_find(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.show_find = !self.show_find;
        if !self.show_find {
            self.find.reset();
        }
    }

    fn do_find_next(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        if self.show_find {
            self.find.find_next(&mut self.buffer);
        }
    }

    fn do_insert_date_time(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.buffer.insert_time_date();
    }

    fn do_quit(&mut self, window: &mut Window, _cx: &mut Context<Self>) {
        if self.buffer.is_dirty() {
            self.show_save_dialog = true;
        } else {
            window.remove_window();
        }
    }
}

fn save_dialog_button(label: &'static str, bg: Rgba) -> Div {
    div()
        .px(px(12.0))
        .py(px(4.0))
        .bg(bg)
        .rounded(px(4.0))
        .text_color(rgb(0xFDFAFE))
        .child(label)
}
