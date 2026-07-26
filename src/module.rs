use std::collections::HashMap;

pub trait Module {
    fn command(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
}

pub struct ModuleRegistry {
    modules: HashMap<&'static str, Box<dyn Module>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    pub fn register(&mut self, module: Box<dyn Module>) {
        let command = module.command().to_string();
        self.modules.insert(command.leak(), module);
    }

    pub fn get(&self, command: &str) -> Option<&Box<dyn Module>> {
        self.modules.get(command)
    }

    pub fn all_commands(&self) -> Vec<&str> {
        self.modules.keys().copied().collect()
    }

    pub fn search(&self, prefix: &str) -> Vec<(&str, &str, &str)> {
        self.modules
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(prefix))
            .map(|(cmd, m)| (*cmd, m.name(), m.description()))
            .collect()
    }
}
