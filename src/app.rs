use gpui::{Context, Entity, Render, Window, div, rgb};
use gpui::prelude::*;

use crate::config::ElementConfig;
use crate::editor::EditorView;
use crate::overlay::OverlayView;
use crate::module::ModuleRegistry;

use std::sync::Arc;

pub struct ElementApp {
    pub config: ElementConfig,
    pub editor: Entity<EditorView>,
    pub overlay: Entity<OverlayView>,
    pub module_registry: Arc<ModuleRegistry>,
}

impl ElementApp {
    pub fn new(
        config: ElementConfig,
        editor: Entity<EditorView>,
        overlay: Entity<OverlayView>,
        module_registry: Arc<ModuleRegistry>,
    ) -> Self {
        Self {
            config,
            editor,
            overlay,
            module_registry,
        }
    }
}

impl Render for ElementApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let overlay_visible = self
            .overlay
            .update(cx, |overlay, _| overlay.is_visible());

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x08060D))
            .child(self.editor.clone())
            .when(overlay_visible, |div| {
                div.child(self.overlay.clone())
            })
    }
}
