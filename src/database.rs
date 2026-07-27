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
            );
            CREATE TABLE IF NOT EXISTS frecency (
                app_name TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 1,
                last_used DATETIME DEFAULT CURRENT_TIMESTAMP
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

    pub fn record_launch(&self, app_name: &str) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        conn.execute(
            "INSERT INTO frecency (app_name, count, last_used) VALUES (?1, 1, CURRENT_TIMESTAMP)
             ON CONFLICT(app_name) DO UPDATE SET count = count + 1, last_used = CURRENT_TIMESTAMP",
            params![app_name],
        ).ok();
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
        }).ok().map(|m| m.filter_map(|r| r.ok()).collect()).unwrap_or_default()
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
