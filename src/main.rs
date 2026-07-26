#![allow(dead_code)]
mod app;
mod config;
mod editor;
mod module;
mod overlay;

use std::sync::Arc;

use gpui::{App, Bounds, Focusable, WindowBounds, WindowOptions, px, size};
use gpui::prelude::*;
use gpui_platform::application;

use app::ElementApp;
use config::ElementConfig;
use editor::buffer::TextBuffer;
use editor::EditorView;
use module::ModuleRegistry;
use overlay::OverlayView;

fn main() {
    application().run(|cx: &mut App| {
        let icon = std::fs::read("brandkit/app-icons/icon-256.png")
            .ok()
            .and_then(|data| image::load_from_memory(&data).ok())
            .map(|img| Arc::new(img.to_rgba8()));

        let bounds = Bounds::centered(None, size(px(920.0), px(660.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                icon,
                focus: true,
                show: true,
                ..Default::default()
            },
            |window, cx| {
                let config = ElementConfig::load();
                let module_registry = Arc::new(ModuleRegistry::new());
                let editor = cx.new(|cx| EditorView::new(TextBuffer::new(), cx));
                let overlay = cx.new(|_| OverlayView::new(module_registry.clone()));
                let app = cx.new(|_| ElementApp::new(config, editor.clone(), overlay, module_registry));
                window.focus(&editor.focus_handle(cx), cx);
                app
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
