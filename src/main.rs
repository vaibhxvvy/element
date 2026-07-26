use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let file_arg = args.get(1).cloned();

    let icon = include_bytes!("../logo.png");
    let icon_data = eframe::icon_data::from_png_bytes(icon).ok();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([920.0, 660.0])
        .with_title("Element");

    if let Some(icon) = icon_data {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Element",
        options,
        Box::new(move |cc| {
            let mut style = (*cc.egui_ctx.style()).clone();
            style.visuals.widgets.noninteractive.bg_fill = egui::Color32::WHITE;
            cc.egui_ctx.set_style(style);
            let mut app = ElementApp::default();
            if let Some(path) = &file_arg {
                if let Ok(content) = std::fs::read_to_string(path) {
                    app.text = content.clone();
                    app.saved_text = content;
                    app.file_path = Some(PathBuf::from(path));
                }
            }
            Ok(Box::new(app))
        }),
    )
}

struct ElementApp {
    text: String,
    saved_text: String,
    file_path: Option<PathBuf>,
    dirty: bool,
    word_wrap: bool,
    show_find: bool,
    find_text: String,
    cursor_offset: usize,
    cursor_line: usize,
    cursor_col: usize,
    show_save_dialog: bool,
    match_count: usize,
    last_find_pos: Option<usize>,
    error_msg: String,
    user_wants_exit: bool,
}

impl Default for ElementApp {
    fn default() -> Self {
        Self {
            text: String::new(),
            saved_text: String::new(),
            file_path: None,
            dirty: false,
            word_wrap: false,
            show_find: false,
            find_text: String::new(),
            cursor_offset: 0,
            cursor_line: 1,
            cursor_col: 1,
            show_save_dialog: false,
            match_count: 0,
            last_find_pos: None,
            error_msg: String::new(),
            user_wants_exit: false,
        }
    }
}

impl eframe::App for ElementApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_close_request(ctx);
        self.handle_shortcuts(ctx);

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                self.file_menu(ui);
                self.edit_menu(ui);
                self.search_menu(ui);
                self.view_menu(ui);
            });
        });

        if self.show_find {
            egui::TopBottomPanel::top("find")
                .min_height(28.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Find:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.find_text)
                                .desired_width(200.0)
                                .hint_text("text to find"),
                        );
                        if ui.button("Find Next").clicked() {
                            self.find_next(ctx);
                        }
                        if self.match_count > 0 {
                            ui.label(format!("({})", self.match_count));
                        }
                        if ui.button("Close").clicked() {
                            self.show_find = false;
                        }
                    });
                });
        }

        egui::TopBottomPanel::bottom("status")
            .min_height(22.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let modified = if self.dirty { " (modified)" } else { "" };
                    ui.label(format!(
                        "Ln {}, Col {}{}",
                        self.cursor_line, self.cursor_col, modified
                    ));
                    if !self.error_msg.is_empty() {
                        ui.colored_label(egui::Color32::RED, &self.error_msg);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !self.word_wrap {
                            ui.label("No Wrap");
                        }
                        if let Some(ref path) = self.file_path {
                            ui.label(path.to_string_lossy().to_string());
                        }
                    });
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut te = egui::TextEdit::multiline(&mut self.text)
                .font(egui::TextStyle::Monospace)
                .code_editor();
            if !self.word_wrap {
                te = te.desired_width(f32::INFINITY);
            }

            let response = te.show(ui);

            self.dirty = self.text != self.saved_text;

            if let Some(cursor_range) = response.state.cursor.char_range() {
                let cursor = cursor_range.primary;
                self.cursor_offset = cursor.index;
                self.cursor_line = 1;
                self.cursor_col = 1;
                for c in self.text[..cursor.index].chars() {
                    if c == '\n' {
                        self.cursor_line += 1;
                        self.cursor_col = 1;
                    } else {
                        self.cursor_col += 1;
                    }
                }
            }
        });

        if self.show_save_dialog {
            egui::Window::new("Element")
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Do you want to save changes?");
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            self.save();
                            if !self.dirty {
                                self.show_save_dialog = false;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        }
                        if ui.button("Don't Save").clicked() {
                            self.show_save_dialog = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_save_dialog = false;
                            self.user_wants_exit = false;
                        }
                    });
                });
        }
    }
}

impl ElementApp {
    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.user_wants_exit {
            if self.dirty {
                self.show_save_dialog = true;
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::N)) {
            self.new_file(ctx);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::O)) {
            self.open_file();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::S)) {
            self.save();
        }
        if ctx.input_mut(|i| {
            i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::S)
        }) {
            self.save_as();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::F)) {
            self.show_find = !self.show_find;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F3)) {
            self.find_next(ctx);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Q)) {
            self.do_exit(ctx);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::D)) {
            self.insert_time_date();
        }
    }

    fn file_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            if ui.button("New    Ctrl+N").clicked() {
                self.new_file(ui.ctx());
                ui.close_menu();
            }
            if ui.button("Open...    Ctrl+O").clicked() {
                self.open_file();
                ui.close_menu();
            }
            if ui.button("Save    Ctrl+S").clicked() {
                self.save();
                ui.close_menu();
            }
            if ui.button("Save As...    Ctrl+Shift+S").clicked() {
                self.save_as();
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Exit    Ctrl+Q").clicked() {
                self.do_exit(ui.ctx());
                ui.close_menu();
            }
        });
    }

    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Edit", |ui| {
            if ui.button("Time/Date    Ctrl+D").clicked() {
                self.insert_time_date();
                ui.close_menu();
            }
        });
    }

    fn search_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Search", |ui| {
            if ui.button("Find...    Ctrl+F").clicked() {
                self.show_find = true;
                ui.close_menu();
            }
            if ui.button("Find Next    F3").clicked() {
                self.find_next(ui.ctx());
                ui.close_menu();
            }
        });
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("View", |ui| {
            if ui.selectable_label(self.word_wrap, "Word Wrap").clicked() {
                self.word_wrap = !self.word_wrap;
                ui.close_menu();
            }
        });
    }

    fn new_file(&mut self, ctx: &egui::Context) {
        if self.dirty {
            self.show_save_dialog = true;
            return;
        }
        self.text.clear();
        self.saved_text.clear();
        self.file_path = None;
        self.dirty = false;
        self.error_msg.clear();
        ctx.data_mut(|d| {
            d.remove::<egui::widgets::text_edit::TextEditState>(egui::Id::new("editor_text"));
        });
    }

    fn open_file(&mut self) {
        if self.dirty {
            return;
        }
        let file = rfd::FileDialog::new()
            .set_title("Open File")
            .add_filter("All Files", &["*"])
            .pick_file();
        if let Some(path) = file {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    self.text = content.clone();
                    self.saved_text = content;
                    self.file_path = Some(path);
                    self.dirty = false;
                    self.error_msg.clear();
                }
                Err(e) => {
                    self.error_msg = format!("Error opening file: {}", e);
                }
            }
        }
    }

    fn save(&mut self) {
        if let Some(path) = &self.file_path {
            match std::fs::write(path, &self.text) {
                Ok(_) => {
                    self.saved_text = self.text.clone();
                    self.dirty = false;
                    self.error_msg.clear();
                }
                Err(e) => {
                    self.error_msg = format!("Error saving: {}", e);
                }
            }
        } else {
            self.save_as();
        }
    }

    fn save_as(&mut self) {
        let file = rfd::FileDialog::new()
            .set_title("Save As")
            .add_filter("All Files", &["*"])
            .save_file();
        if let Some(path) = file {
            match std::fs::write(&path, &self.text) {
                Ok(_) => {
                    self.file_path = Some(path);
                    self.saved_text = self.text.clone();
                    self.dirty = false;
                    self.error_msg.clear();
                }
                Err(e) => {
                    self.error_msg = format!("Error saving: {}", e);
                }
            }
        }
    }

    fn do_exit(&mut self, ctx: &egui::Context) {
        if self.dirty {
            self.show_save_dialog = true;
            self.user_wants_exit = true;
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn find_next(&mut self, ctx: &egui::Context) {
        if self.find_text.is_empty() {
            self.match_count = 0;
            return;
        }

        let search_from = match self.last_find_pos {
            Some(pos) if pos + self.find_text.len() < self.text.len() => {
                pos + self.find_text.len()
            }
            _ => 0,
        };

        let search = &self.text[search_from..];
        if let Some(pos) = search.find(&self.find_text) {
            let abs_pos = search_from + pos;
            self.last_find_pos = Some(abs_pos);
            self.match_count = self.text.matches(&self.find_text).count();
            self.set_cursor(ctx, abs_pos);
        } else if !self.text.is_empty() {
            let end = search_from.saturating_sub(1);
            let search = &self.text[..end];
            if let Some(pos) = search.find(&self.find_text) {
                self.last_find_pos = Some(pos);
                self.set_cursor(ctx, pos);
            } else {
                self.last_find_pos = None;
                self.match_count = 0;
            }
        }
    }

    fn set_cursor(&self, ctx: &egui::Context, byte_offset: usize) {
        use egui::text::CCursorRange;
        use egui::widgets::text_edit::TextEditState;
        let id = egui::Id::new("editor_text");
        let cursor_range = CCursorRange::one(
            egui::text::CCursor::new(byte_offset),
        );
        let mut state = TextEditState::load(ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(cursor_range));
        state.store(ctx, id);
    }

    fn insert_time_date(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let secs = now % 60;
        let mins = (now / 60) % 60;
        let hours = (now / 3600) % 24;
        let day = (now / 86400) % 31 + 1;
        let month = ((now / 86400 / 30) % 12) + 1;
        let year = 2026;
        let date_str = format!("{:02}:{:02}:{:02} {:02}/{:02}/{}", hours, mins, secs, day, month, year);
        self.text.insert_str(self.cursor_offset, &date_str);
        self.cursor_offset += date_str.len();
        self.dirty = true;
    }
}
