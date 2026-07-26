use gpui::{Context, Render, Window, div, hsla, rgb, px};
use gpui::prelude::*;
use std::sync::Arc;

use crate::module::ModuleRegistry;

pub struct OverlayView {
    visible: bool,
    search_query: String,
    module_registry: Arc<ModuleRegistry>,
    results: Vec<(String, String, String)>,
}

impl OverlayView {
    pub fn new(module_registry: Arc<ModuleRegistry>) -> Self {
        Self {
            visible: false,
            search_query: String::new(),
            module_registry,
            results: Vec::new(),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.search_query.clear();
            self.results.clear();
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    fn on_input(&mut self, text: &str) {
        self.search_query = text.to_string();
        self.results.clear();

        if text.starts_with('/') {
            let modules = self.module_registry.search(text);
            self.results = modules
                .into_iter()
                .map(|(cmd, name, desc)| (cmd.to_string(), name.to_string(), desc.to_string()))
                .collect();
        }
    }
}

impl Render for OverlayView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible {
            return div().size_full();
        }

        div()
            .flex()
            .size_full()
            .justify_center()
            .items_start()
            .pt(px(120.0))
            .bg(hsla(0.0, 0.0, 0.0, 0.5))
            .child(
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
                            .child(div().flex().w_full().child(
                                self.search_query.clone(),
                            )),
                    )
                    .children(self.results.iter().map(|(cmd, name, _desc)| {
                        div()
                            .flex()
                            .px(px(16.0))
                            .py(px(8.0))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(12.0))
                                    .child(
                                        div()
                                            .text_color(rgb(0x6D4AFA))
                                            .child(cmd.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_color(rgb(0xA7A0BD))
                                            .child(name.clone()),
                                    ),
                            )
                    })),
            )
    }
}
