use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::Mutex;

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
            );"
        ).ok();
        Self { conn: Mutex::new(conn) }
    }

    fn db_path() -> PathBuf {
        let mut path = dirs_data_dir();
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
        }).ok().map(|m| m.filter_map(|r| r.ok()).collect()).unwrap_or_default()
    }
}

fn dirs_data_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".element");
        p
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        let mut p = PathBuf::from(profile);
        p.push(".element");
        p
    } else {
        PathBuf::from(".element")
    }
}
