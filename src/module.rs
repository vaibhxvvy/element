use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, Entity, Window};

use crate::database::Database;
use crate::editor::EditorView;
use crate::overlay::OverlayView;

pub type ActivateFn =
    Arc<dyn Fn(Entity<EditorView>, Entity<OverlayView>, &mut Window, &mut App)>;

#[derive(Clone)]
pub struct Module {
    pub command: String,
    pub name: String,
    pub description: String,
    pub activate: ActivateFn,
}

impl Module {
    pub fn new(
        command: &str,
        name: &str,
        description: &str,
        activate: ActivateFn,
    ) -> Self {
        Self {
            command: command.into(),
            name: name.into(),
            description: description.into(),
            activate,
        }
    }
}

pub struct ModuleRegistry {
    modules: RefCell<HashMap<String, Module>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: RefCell::new(HashMap::new()),
        }
    }

    pub fn register(&self, module: Module) {
        self.modules
            .borrow_mut()
            .insert(module.command.clone(), module);
    }

    pub fn search(&self, prefix: &str) -> Vec<(String, String, String)> {
        self.modules
            .borrow()
            .values()
            .filter(|m| m.command.starts_with(prefix))
            .map(|m| (m.command.clone(), m.name.clone(), m.description.clone()))
            .collect()
    }

    pub fn activate(
        &self,
        command: &str,
        editor: Entity<EditorView>,
        overlay: Entity<OverlayView>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(module) = self.modules.borrow().get(command) {
            (module.activate)(editor, overlay, window, cx);
        }
    }
}

pub fn register_builtin_modules(
    registry: &ModuleRegistry,
    _editor: Entity<EditorView>,
    _overlay: Entity<OverlayView>,
    db: Arc<Database>,
) {
    registry.register(Module::new(
        "/notes",
        "Notes",
        "Open a new note in the editor",
        Arc::new(move |editor: Entity<EditorView>,
                       _overlay: Entity<OverlayView>,
                       _window: &mut gpui::Window,
                       cx: &mut App| {
            let _ = editor.update(cx, |ed, _cx| {
                ed.buffer =
                    crate::editor::buffer::TextBuffer::from_string("# New Note\n\n".into());
            });
        }),
    ));

    registry.register(Module::new(
        "/help",
        "Help",
        "Show available commands",
        Arc::new(move |editor: Entity<EditorView>,
                       _overlay: Entity<OverlayView>,
                       _window: &mut gpui::Window,
                       cx: &mut App| {
            let _ = editor.update(cx, |ed, _cx| {
                ed.buffer = crate::editor::buffer::TextBuffer::from_string(
                    "# Element Commands\n\n\
                     /notes  - Open a new note\n\
                     /help   - Show this help\n\
                     /calc   - Calculate a math expression\n\
                     /cbhist - Clipboard history\n\
                     /emoji  - Search emojis\n\
                     /savenote - Save current buffer as a note\n\
                     \n\
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
        }),
    ));

    registry.register(Module::new(
        "/calc",
        "Calculator",
        "Evaluate a math expression like 2+2 or sqrt(144)",
        Arc::new(move |editor: Entity<EditorView>,
                       _overlay: Entity<OverlayView>,
                       _window: &mut gpui::Window,
                       cx: &mut App| {
            let _ = editor.update(cx, |ed, _cx| {
                let result = evalexpr::eval(
                    &ed.buffer.text.trim().replace('x', "*").replace('÷', "/"),
                )
                .map(|v| format!("= {}", v))
                .unwrap_or_else(|e| format!("Error: {}", e));
                ed.buffer = crate::editor::buffer::TextBuffer::from_string(
                    format!("{} {}\n", ed.buffer.text.trim(), result),
                );
            });
        }),
    ));

    let db_cbhist = db.clone();
    registry.register(Module::new(
        "/cbhist",
        "Clipboard History",
        "Browse recent clipboard entries",
        Arc::new(move |editor: Entity<EditorView>,
                       _overlay: Entity<OverlayView>,
                       _window: &mut gpui::Window,
                       cx: &mut App| {
            let entries = db_cbhist.load_clipboard(30);
            let content: String = if entries.is_empty() {
                "# Clipboard History\n\n(empty — copy something first)".into()
            } else {
                let mut s = "# Clipboard History\n\n".to_string();
                for (i, (text, _ts)) in entries.iter().enumerate() {
                    let preview: String = text.lines().next().unwrap_or(text).chars().take(80).collect();
                    s.push_str(&format!("{}. {}\n", i + 1, preview));
                }
                s
            };
            let _ = editor.update(cx, |ed, _cx| {
                ed.buffer = crate::editor::buffer::TextBuffer::from_string(content);
            });
        }),
    ));

    registry.register(Module::new(
        "/emoji",
        "Emoji Search",
        "Search and insert emojis",
        Arc::new(move |editor: Entity<EditorView>,
                       _overlay: Entity<OverlayView>,
                       _window: &mut gpui::Window,
                       cx: &mut App| {
            let query = std::env::var("EMOJI_QUERY").unwrap_or_default();
            let mut results = String::from("# Emojis\n\n");
            let query_lc = query.to_lowercase();
            for emoji in emojis::iter() {
                if query_lc.is_empty()
                    || emoji.name().to_lowercase().contains(&query_lc)
                    || emoji.shortcodes().any(|s| s.contains(&query_lc))
                {
                    let shortcodes: Vec<&str> = emoji.shortcodes().collect();
                    let code = shortcodes.first().copied().unwrap_or("");
                    results.push_str(&format!("{}  :{}:\n", emoji.as_str(), code));
                }
                if results.len() > 2000 {
                    results.push_str("\n... (truncated)");
                    break;
                }
            }
            if results.lines().count() <= 1 {
                results.push_str("(no emojis found)");
            }
            let _ = editor.update(cx, |ed, _cx| {
                ed.buffer = crate::editor::buffer::TextBuffer::from_string(results);
            });
        }),
    ));

    let db_save = db.clone();
    registry.register(Module::new(
        "/savenote",
        "Save Note",
        "Save the current editor content as a persistent note",
        Arc::new(move |editor: Entity<EditorView>,
                       _overlay: Entity<OverlayView>,
                       _window: &mut gpui::Window,
                       cx: &mut App| {
            let _ = editor.update(cx, |ed, _cx| {
                let text = ed.buffer.text.clone();
                let title = text.lines().next().unwrap_or("Untitled").to_string();
                let title = title.trim_start_matches("# ").to_string();
                db_save.store_note(&title, &text);
                ed.buffer = crate::editor::buffer::TextBuffer::from_string(
                    format!("✓ Note saved: {}\n", title),
                );
            });
        }),
    ));

    let db_notes = db.clone();
    registry.register(Module::new(
        "/noteslist",
        "List Notes",
        "Show all saved notes",
        Arc::new(move |editor: Entity<EditorView>,
                       _overlay: Entity<OverlayView>,
                       _window: &mut gpui::Window,
                       cx: &mut App| {
            let notes = db_notes.load_notes(50);
            let content: String = if notes.is_empty() {
                "# Saved Notes\n\n(no notes saved yet — use /savenote)".into()
            } else {
                let mut s = "# Saved Notes\n\n".to_string();
                for (title, _content) in &notes {
                    s.push_str(&format!("- {}\n", title));
                }
                s
            };
            let _ = editor.update(cx, |ed, _cx| {
                ed.buffer = crate::editor::buffer::TextBuffer::from_string(content);
            });
        }),
    ));
}
