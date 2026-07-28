use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::config;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new() -> Self {
        let path = Self::db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&path).unwrap_or_else(|e| {
            eprintln!("Failed to open database: {:?}", e);
            std::process::exit(1);
        });
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text',
                text_content TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS frecency (
                app_name TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 1,
                last_used DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .ok();
        Self {
            conn: Mutex::new(conn),
        }
    }

    fn db_path() -> std::path::PathBuf {
        let mut path = config::data_dir();
        path.push("element.db");
        path
    }

    pub fn load_clipboard(&self, limit: usize) -> Vec<(String, String)> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut stmt = match conn.prepare(
            "SELECT text_content, created_at FROM clipboard_entries WHERE text_content IS NOT NULL ORDER BY id DESC LIMIT ?1"
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0).unwrap_or_default(),
                row.get::<_, String>(1).unwrap_or_default(),
            ))
        })
        .ok()
        .map(|m| m.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Create a temporary in-memory database for testing.
    #[cfg(test)]
    pub(crate) fn new_in_memory() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text',
                text_content TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS frecency (
                app_name TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 1,
                last_used DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .ok();
        Self {
            conn: Mutex::new(conn),
        }
    }

    pub fn record_launch(&self, app_name: &str) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        conn.execute(
            "INSERT INTO frecency (app_name, count, last_used) VALUES (?1, 1, CURRENT_TIMESTAMP)
             ON CONFLICT(app_name) DO UPDATE SET count = count + 1, last_used = CURRENT_TIMESTAMP",
            params![app_name],
        )
        .ok();
    }

    pub fn top_frecency(&self, limit: usize) -> Vec<(String, i64, String)> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut stmt = match conn.prepare(
            "SELECT app_name, count, last_used FROM frecency ORDER BY count * (1.0 / (julianday('now') - julianday(last_used) + 1)) DESC LIMIT ?1"
        ) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0).unwrap_or_default(),
                row.get::<_, i64>(1).unwrap_or(0),
                row.get::<_, String>(2).unwrap_or_default(),
            ))
        })
        .ok()
        .map(|m| m.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Return raw count for an app (used by tests).
    #[cfg(test)]
    pub(crate) fn frecency_count(&self, app_name: &str) -> i64 {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return 0,
        };
        conn.query_row(
            "SELECT count FROM frecency WHERE app_name = ?1",
            params![app_name],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
    }

    pub fn frecency_score(&self, app_name: &str) -> f64 {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return 0.0,
        };
        conn.query_row(
            "SELECT count * (1.0 / (julianday('now') - julianday(last_used) + 1)) FROM frecency WHERE app_name = ?1",
            params![app_name],
            |row| row.get::<_, f64>(0),
        ).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frecency_starts_at_zero() {
        let db = Database::new_in_memory();
        assert_eq!(db.frecency_score("nonexistent"), 0.0);
        assert_eq!(db.frecency_count("nonexistent"), 0);
    }

    #[test]
    fn frecency_increments_on_launch() {
        let db = Database::new_in_memory();
        db.record_launch("Calc");
        assert_eq!(db.frecency_count("Calc"), 1);
        let s1 = db.frecency_score("Calc");
        assert!(s1 > 0.0);

        db.record_launch("Calc");
        assert_eq!(db.frecency_count("Calc"), 2);
        let s2 = db.frecency_score("Calc");
        assert!(s2 > s1, "second launch should increase score");
    }

    #[test]
    fn frecency_multiple_apps() {
        let db = Database::new_in_memory();
        db.record_launch("AppA");
        db.record_launch("AppA");
        db.record_launch("AppB");
        let top = db.top_frecency(5);
        assert_eq!(top.len(), 2);
        // AppA should rank higher (2 launches vs 1)
        let names: Vec<&str> = top.iter().map(|(n, _, _)| n.as_str()).collect();
        assert_eq!(names[0], "AppA");
        assert_eq!(names[1], "AppB");
    }

    #[test]
    fn frecency_top_respects_limit() {
        let db = Database::new_in_memory();
        db.record_launch("A");
        db.record_launch("B");
        db.record_launch("C");
        let top = db.top_frecency(2);
        assert_eq!(top.len(), 2);
    }

    #[test]
    fn clipboard_empty_initially() {
        let db = Database::new_in_memory();
        let entries = db.load_clipboard(10);
        assert!(entries.is_empty());
    }

    #[test]
    fn clipboard_returns_newest_first() {
        let db = Database::new_in_memory();
        // Manually insert entries with explicit timestamps to test ordering
        {
            let conn = db.conn.lock().unwrap();
            conn.execute_batch(
                "INSERT INTO clipboard_entries (text_content, created_at)
                 VALUES ('first',  '2024-01-01 00:00:00');
                 INSERT INTO clipboard_entries (text_content, created_at)
                 VALUES ('second', '2024-01-02 00:00:00');
                 INSERT INTO clipboard_entries (text_content, created_at)
                 VALUES ('third',  '2024-01-03 00:00:00');",
            )
            .unwrap();
        }
        let entries = db.load_clipboard(10);
        assert_eq!(entries.len(), 3);
        // Should be newest first
        assert!(entries[0].0.contains("third"));
        assert!(entries[1].0.contains("second"));
        assert!(entries[2].0.contains("first"));
    }

    #[test]
    fn clipboard_respects_limit() {
        let db = Database::new_in_memory();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute_batch(
                "INSERT INTO clipboard_entries (text_content)
                 VALUES ('one'), ('two'), ('three'), ('four'), ('five');",
            )
            .unwrap();
        }
        let entries = db.load_clipboard(3);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn frecency_score_increases_with_usage() {
        let db = Database::new_in_memory();
        // Score = count / (days_since_last_use + 1)
        db.record_launch("Notes");
        let score1 = db.frecency_score("Notes");
        assert!(score1 > 0.0, "score should be positive after first use");

        db.record_launch("Notes");
        let score2 = db.frecency_score("Notes");
        assert!(
            score2 >= score1,
            "score should not decrease: {} < {}",
            score2,
            score1
        );
    }
}
