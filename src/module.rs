use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, Entity, Window};

use crate::editor::EditorView;
use crate::overlay::OverlayView;

pub type ActivateFn =
    Arc<dyn Fn(Entity<EditorView>, Entity<OverlayView>, &mut Window, &mut App)>;

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
