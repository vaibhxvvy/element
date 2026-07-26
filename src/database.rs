use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

pub struct Database {
    pub conn: Connection,
}

impl Database {
    pub fn init() -> Self {
        let path = Self::db_path();
        std::fs::create_dir_all(path.parent().unwrap()).ok();
        let conn = Connection::open(&path).expect("Failed to open database");
        let db = Self { conn };
        db.migrate();
        db
    }

    fn db_path() -> PathBuf {
        let mut path = data_dir();
        path.push("element");
        std::fs::create_dir_all(&path).ok();
        path.push("element.db");
        path
    }

    fn migrate(&self) {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                text_content TEXT,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        ).ok();
    }

    pub fn store_clipboard(&self, text: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.conn
            .execute(
                "INSERT INTO clipboard_entries (content_type, text_content, created_at)
                 VALUES ('text', ?1, ?2)",
                params![text, now],
            )
            .ok();
    }

    pub fn load_clipboard(&self, limit: u32) -> Vec<(String, String)> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT text_content, created_at
                 FROM clipboard_entries
                 WHERE content_type = 'text'
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )
            .unwrap_or_else(|_| panic!("Failed to prepare clipboard query"));

        stmt.query_map([limit], |row| {
            let text: String = row.get(0)?;
            let ts: i64 = row.get(1)?;
            Ok((text, ts.to_string()))
        })
        .unwrap_or_else(|_| panic!("Failed to query clipboard"))
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn store_note(&self, title: &str, content: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.conn
            .execute(
                "INSERT INTO notes (title, content, updated_at) VALUES (?1, ?2, ?3)",
                params![title, content, now],
            )
            .ok();
    }

    pub fn load_notes(&self, limit: u32) -> Vec<(String, String)> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT title, content FROM notes ORDER BY updated_at DESC LIMIT ?1",
            )
            .unwrap_or_else(|_| panic!("Failed to prepare notes query"));

        stmt.query_map([limit], |row| {
            let title: String = row.get(0)?;
            let content: String = row.get(1)?;
            Ok((title, content))
        })
        .unwrap_or_else(|_| panic!("Failed to query notes"))
        .filter_map(|r| r.ok())
        .collect()
    }
}

fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var("ELEMENT_DATA_DIR")
        .ok()
        .map(PathBuf::from)
    {
        return dir;
    }
    if let Some(dir) = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
    {
        return dir;
    }
    PathBuf::from(".")
}
