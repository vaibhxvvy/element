mod app;
mod config;
mod editor;
mod module;
mod overlay;

use std::sync::Arc;

use gpui::{App, Bounds, WindowBounds, WindowOptions, px, size};
use gpui::prelude::*;
use gpui_platform::application;

use app::ElementApp;
use config::ElementConfig;
use editor::EditorView;
use module::ModuleRegistry;
use overlay::OverlayView;

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(920.0), px(660.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_window, cx| {
                let config = ElementConfig::load();
                let module_registry = Arc::new(ModuleRegistry::new());
                let editor = cx.new(|_| EditorView::new(Default::default()));
                let overlay = cx.new(|_| OverlayView::new(module_registry.clone()));
                cx.new(|_| ElementApp::new(config, editor, overlay, module_registry))
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
