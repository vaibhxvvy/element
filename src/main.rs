#![allow(dead_code)]
mod app;
mod config;
mod editor;
mod module;
mod overlay;

use std::sync::Arc;
use std::time::Duration;

use global_hotkey::hotkey::{HotKey, Modifiers, Code};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui::{App, Bounds, Entity, Focusable, WindowBounds, WindowOptions, px, size};
use gpui::prelude::*;
use gpui_platform::application;

use app::ElementApp;
use config::ElementConfig;
use editor::buffer::TextBuffer;
use editor::EditorView;
use module::{Module, ModuleRegistry};
use overlay::OverlayView;

fn main() {
    application().run(|cx: &mut App| {
        let config = ElementConfig::load();
        let hotkey = config.hotkey.clone();

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

                module_registry.register(
                    Module::new(
                        "/notes",
                        "Notes",
                        "Open a new note in the editor",
                        Arc::new(
                            |editor: Entity<EditorView>,
                             _overlay: Entity<OverlayView>,
                             _window: &mut gpui::Window,
                             cx: &mut App| {
                                let _ = editor.update(cx, |ed, _cx| {
                                    ed.buffer =
                                        TextBuffer::from_string("# New Note\n\n".into());
                                });
                            },
                        ),
                    ),
                );

                module_registry.register(
                    Module::new(
                        "/help",
                        "Help",
                        "Show available commands",
                        Arc::new(
                            |editor: Entity<EditorView>,
                             _overlay: Entity<OverlayView>,
                             _window: &mut gpui::Window,
                             cx: &mut App| {
                                let _ = editor.update(cx, |ed, _cx| {
                                    ed.buffer = TextBuffer::from_string(
                                        "# Element Commands\n\n\
                                         /notes  - Open a new note\n\
                                         /help   - Show this help\n\n\
                                         Ctrl+N  - New file\n\
                                         Ctrl+O  - Open file\n\
                                         Ctrl+S  - Save\n\
                                         Ctrl+F  - Find\n\
                                         F3      - Find next\n\
                                         Ctrl+Q  - Quit\n\
                                         Alt+Space - Toggle overlay\n"
                                            .into(),
                                    );
                                });
                            },
                        ),
                    ),
                );

                overlay.update(cx, |o, _cx| {
                    o.set_handles(editor.clone(), overlay.clone());
                });

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
