use gpui::*;

use std::sync::Arc;

use crate::editor::EditorView;
use crate::module::ModuleRegistry;

pub struct OverlayView {
    pub visible: bool,
    was_visible: bool,
    search_query: String,
    module_registry: Arc<ModuleRegistry>,
    results: Vec<(String, String, String)>,
    selected_index: usize,
    pub editor_handle: Option<Entity<EditorView>>,
    overlay_handle: Option<Entity<OverlayView>>,
    focus_handle: FocusHandle,
}

impl OverlayView {
    pub fn new(module_registry: Arc<ModuleRegistry>, cx: &mut Context<Self>) -> Self {
        Self {
            visible: false,
            was_visible: false,
            search_query: String::new(),
            module_registry,
            results: Vec::new(),
            selected_index: 0,
            editor_handle: None,
            overlay_handle: None,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_handles(
        &mut self,
        editor: Entity<EditorView>,
        overlay: Entity<OverlayView>,
    ) {
        self.editor_handle = Some(editor);
        self.overlay_handle = Some(overlay);
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.search_query.clear();
            self.results.clear();
            self.selected_index = 0;
        }
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn search(&mut self) {
        self.results.clear();
        self.selected_index = 0;

        if self.search_query.starts_with('/') {
            self.results = self.module_registry.search(&self.search_query);
        }
    }

    fn activate_selected(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.results.is_empty() {
            return;
        }

        let idx = self.selected_index.min(self.results.len().saturating_sub(1));
        let (cmd, _, _) = &self.results[idx];
        let cmd = cmd.clone();
        self.visible = false;

        if let Some(ref editor) = self.editor_handle.clone() {
            if let Some(ref overlay_h) = self.overlay_handle.clone() {
                self.module_registry.activate(
                    &cmd,
                    editor.clone(),
                    overlay_h.clone(),
                    window,
                    cx,
                );
                let fh = editor.update(cx, |ed, _cx| ed.focus_handle.clone());
                window.focus(&fh, cx);
            }
        }
        cx.notify();
    }

    fn render_panel(&self) -> impl IntoElement {
        let query_text: String = if self.search_query.is_empty() {
            "Type a command or / for modules...".to_string()
        } else {
            self.search_query.clone()
        };

        div()
            .flex()
            .flex_col()
            .w(px(560.0))
            .rounded(px(12.0))
            .bg(rgb(0x12101A))
            .shadow_lg()
            .child(
                div()
                    .flex()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(rgb(0x2A2740))
                    .child(
                        div()
                            .flex()
                            .w_full()
                            .text_color(rgb(0xFDFAFE))
                            .text_size(px(14.0))
                            .child(query_text),
                    ),
            )
            .children(self.render_results())
    }

    fn render_results(&self) -> Vec<gpui::Div> {
        self.results
            .iter()
            .enumerate()
            .map(|(i, (cmd, name, _desc))| {
                let is_selected = i == self.selected_index;
                let bg = if is_selected {
                    rgb(0x6D4AFA)
                } else {
                    rgb(0x1C1A2A)
                };
                div()
                    .flex()
                    .px(px(16.0))
                    .py(px(10.0))
                    .bg(bg)
                    .cursor_pointer()
                    .child(
                        div()
                            .flex()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .text_color(rgb(0xFDFAFE))
                                    .text_size(px(13.0))
                                    .font_weight(if is_selected {
                                        FontWeight::BOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .child(cmd.clone()),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0xA7A0BD))
                                    .text_size(px(13.0))
                                    .child(name.clone()),
                            ),
                    )
            })
            .collect()
    }
}

impl Focusable for OverlayView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for OverlayView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            self.was_visible = false;
            return div().size_full();
        }

        if !self.was_visible {
            self.was_visible = true;
            window.focus(&self.focus_handle, cx);
        }

        let fh = self.focus_handle.clone();

        div()
            .flex()
            .size_full()
            .justify_center()
            .items_start()
            .pt(px(120.0))
            .bg(hsla(0.0, 0.0, 0.0, 0.55))
            .track_focus(&fh)
            .on_key_down(cx.listener(
                |this, event: &KeyDownEvent, window, cx| {
                    let ks = &event.keystroke;
                    match ks.key.as_str() {
                        "escape" => {
                            this.visible = false;
                            if let Some(ref editor) = this.editor_handle {
                                let fh2 =
                                    editor.update(cx, |ed, _cx| ed.focus_handle.clone());
                                window.focus(&fh2, cx);
                            }
                            cx.notify();
                        }
                        "backspace" => {
                            this.search_query.pop();
                            this.search();
                            cx.notify();
                        }
                        "enter" => {
                            this.activate_selected(window, cx);
                            cx.notify();
                        }
                        "up" => {
                            this.selected_index =
                                this.selected_index.saturating_sub(1);
                            cx.notify();
                        }
                        "down" => {
                            this.selected_index = (this.selected_index + 1)
                                .min(this.results.len().saturating_sub(1));
                            cx.notify();
                        }
                        _ => {
                            if let Some(ch) = &ks.key_char {
                                for c in ch.chars() {
                                    if !c.is_control() {
                                        this.search_query.push(c);
                                    }
                                }
                                this.search();
                                cx.notify();
                            }
                        }
                    }
                },
            ))
            .child(self.render_panel())
    }
}
