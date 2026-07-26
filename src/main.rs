#![allow(dead_code)]
mod app;
mod clipboard;
mod config;
mod database;
mod editor;
mod module;
mod overlay;

use std::sync::Arc;
use std::time::Duration;

use global_hotkey::hotkey::{HotKey, Modifiers, Code};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui::{App, Bounds, Focusable, WindowBounds, WindowOptions, px, size};
use gpui::prelude::*;
use gpui_platform::application;

use app::ElementApp;
use config::ElementConfig;
use database::Database;
use editor::buffer::TextBuffer;
use editor::EditorView;
use module::ModuleRegistry;
use overlay::OverlayView;

fn main() {
    application().run(|cx: &mut App| {
        let config = ElementConfig::load();
        let hotkey = config.hotkey.clone();
        let db = Arc::new(Database::init());

        let icon = std::fs::read("brandkit/app-icons/icon-256.png")
            .ok()
            .and_then(|data| image::load_from_memory(&data).ok())
            .map(|img| Arc::new(img.to_rgba8()));

        let bounds = Bounds::centered(
            None,
            size(px(config.window_width), px(config.window_height)),
            cx,
        );
        let _window_handle = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                icon,
                focus: true,
                show: true,
                ..Default::default()
            },
            |window, cx| {
                let module_registry = Arc::new(ModuleRegistry::new());
                let editor = cx.new(|cx| EditorView::new(TextBuffer::new(), cx));
                let overlay = cx.new(|cx| OverlayView::new(module_registry.clone(), cx));

                module::register_builtin_modules(
                    &module_registry,
                    editor.clone(),
                    overlay.clone(),
                    db.clone(),
                );

                overlay.update(cx, |o, _cx| {
                    o.set_handles(editor.clone(), overlay.clone());
                });

                if config.clipboard.enabled {
                    let (clip_tx, clip_rx) = std::sync::mpsc::channel();
                    clipboard::start_clipboard_monitor(
                        Database::init(),
                        clip_tx,
                    );
                    let overlay_clip = overlay.clone();
                    window.spawn(cx, async move |cx| {
                        loop {
                            cx.background_executor()
                                .timer(Duration::from_millis(100))
                                .await;
                            while let Ok(_text) = clip_rx.try_recv() {
                                let _ = overlay_clip.update(cx, |_o, _cx| {});
                            }
                        }
                    })
                    .detach();
                }

                let overlay_handle = overlay.clone();
                window.spawn(cx, async move |cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(50))
                            .await;
                        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
                            if event.state() == HotKeyState::Pressed {
                                let _ = overlay_handle.update(cx, |o, _cx| {
                                    o.toggle();
                                });
                            }
                        }
                    }
                })
                .detach();

                let focus_handle = editor.focus_handle(cx);
                window.focus(&focus_handle, cx);

                cx.new(|_| ElementApp::new(config, editor, overlay, module_registry))
            },
        )
        .unwrap();

        let _ = GlobalHotKeyManager::new().map(|manager| {
            let hotkey_str = hotkey;
            if let Ok(hotkey) = hotkey_str.parse::<HotKey>() {
                let _ = manager.register(hotkey);
            } else {
                let fallback = HotKey::new(Some(Modifiers::ALT), Code::Space);
                let _ = manager.register(fallback);
            }
        });

        cx.activate(true);
    });
}
