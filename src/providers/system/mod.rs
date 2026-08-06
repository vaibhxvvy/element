//! System commands provider — `shutdown`, `restart`, `sleep`, `lock`.
//!
//! Matches the command name at the start of the query (word boundary), so
//! `lock` works and `locksmith` does not. Shutdown/restart shell out to
//! `shutdown.exe`; lock and sleep use Win32 via [`crate::platform`].

use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider, SearchResult};

/// (query keyword, display title, subtitle, action key)
const COMMANDS: &[(&str, &str, &str, &str)] = &[
    ("shutdown", "Shut down", "Turn the computer off", "shutdown"),
    ("restart", "Restart", "Restart the computer", "restart"),
    ("reboot", "Restart", "Restart the computer", "restart"),
    ("sleep", "Sleep", "Put the computer to sleep", "sleep"),
    ("lock", "Lock", "Lock the computer", "lock"),
];

fn parse(query: &str) -> Option<&'static (&'static str, &'static str, &'static str, &'static str)> {
    let q = query.trim().to_lowercase();
    COMMANDS
        .iter()
        .find(|(keyword, _, _, _)| q == *keyword || q.starts_with(&format!("{} ", keyword)))
}

pub struct SystemProvider;

impl SearchProvider for SystemProvider {
    fn id(&self) -> &'static str {
        "system"
    }

    fn priority(&self) -> i32 {
        9
    }

    fn should_run(&self, query: &str) -> bool {
        parse(query).is_some()
    }

    fn search(&self, _ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let Some((_, title, subtitle, _)) = parse(query) else {
            return Vec::new();
        };
        vec![SearchResult {
            title: (*title).into(),
            subtitle: (*subtitle).into(),
            kind: "system".into(),
            provider_id: "system".into(),
            action: COMMANDS
                .iter()
                .find(|(keyword, _, _, _)| query.trim().to_lowercase().starts_with(keyword))
                .map(|(_, _, _, action)| (*action).to_string())
                .unwrap_or_default(),
            icon_rgba: None,
            score: 300.0,
        }]
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        match result.action.as_str() {
            "lock" => crate::platform::lock_workstation().map_err(ElementError::Other),
            "sleep" => crate::platform::suspend_system().map_err(ElementError::Other),
            "shutdown" => run_shutdown("/s"),
            "restart" => run_shutdown("/r"),
            other => Err(ElementError::Other(format!(
                "unknown system command: {}",
                other
            ))),
        }
    }
}

fn run_shutdown(flag: &str) -> Result<(), ElementError> {
    std::process::Command::new("shutdown.exe")
        .args([flag, "/t", "0"])
        .spawn()
        .map(|_| ())
        .map_err(ElementError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_command_words_only() {
        let provider = SystemProvider;
        assert!(provider.should_run("lock"));
        assert!(provider.should_run("LOCK"));
        assert!(provider.should_run("restart"));
        assert!(provider.should_run("shutdown now"));
        assert!(!provider.should_run("locksmith"));
        assert!(!provider.should_run("shut"));
        assert!(!provider.should_run(""));
    }

    #[test]
    fn search_returns_matching_command() {
        let config = crate::config::Config::default();
        let db = crate::database::Database::new_in_memory();
        let ctx = SearchContext {
            config: &config,
            db: &db,
        };
        let provider = SystemProvider;
        let results = provider.search(&ctx, "lock");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Lock");
        assert_eq!(results[0].action, "lock");
        assert_eq!(results[0].score, 300.0);

        // "reboot" maps to the restart action.
        let results = provider.search(&ctx, "reboot");
        assert_eq!(results[0].action, "restart");
    }
}
