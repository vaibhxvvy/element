use rusqlite::{params, params_from_iter, Connection};
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
            "            CREATE TABLE IF NOT EXISTS clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text',
                text_content TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                pinned INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS clipboard_images (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                hash TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                pinned INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS frecency (
                app_name TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 1,
                last_used DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS file_frecency (
                path TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 1,
                last_used DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS emoji_frecency (
                emoji TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 1,
                last_used DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .ok();
        // Migration for databases created before the `pinned` column existed.
        let _ = conn.execute(
            "ALTER TABLE clipboard_entries ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0",
            [],
        );
        Self {
            conn: Mutex::new(conn),
        }
    }

    fn db_path() -> std::path::PathBuf {
        let mut path = config::data_dir();
        path.push("element.db");
        path
    }

    /// Clipboard history: `(text, created_at, pinned, id)`, pinned entries
    /// first, then newest first. `id` is the stable capture order — two rows
    /// captured within the same second share a `created_at`, so sorting by
    /// timestamp alone can put the newest entry second. (Test helper; the
    /// provider uses [`Self::load_clipboard_filtered`].)
    #[cfg(test)]
    pub fn load_clipboard(&self, limit: usize) -> Vec<(String, String, bool, i64)> {
        self.load_clipboard_filtered(limit, None, None, None, true)
    }

    /// Clipboard history with optional text (LIKE) and local-date-range
    /// filters. `date_from`/`date_to` are `YYYY-MM-DD` strings (see
    /// [`Self::local_date`]); `newest_first` flips the id order.
    pub fn load_clipboard_filtered(
        &self,
        limit: usize,
        text_like: Option<&str>,
        date_from: Option<&str>,
        date_to: Option<&str>,
        newest_first: bool,
    ) -> Vec<(String, String, bool, i64)> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let order = if newest_first { "DESC" } else { "ASC" };
        let mut sql = "SELECT text_content, created_at, pinned, id \
                       FROM clipboard_entries WHERE text_content IS NOT NULL"
            .to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut n = 0usize;
        if let Some(term) = text_like {
            n += 1;
            sql.push_str(&format!(" AND text_content LIKE ?{n} ESCAPE '\\'"));
            params.push(Box::new(format!(
                "%{}%",
                term.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )));
        }
        if let Some(from) = date_from {
            n += 1;
            sql.push_str(&format!(
                " AND date(datetime(created_at, 'localtime')) >= ?{n}"
            ));
            params.push(Box::new(from.to_string()));
        }
        if let Some(to) = date_to {
            n += 1;
            sql.push_str(&format!(
                " AND date(datetime(created_at, 'localtime')) <= ?{n}"
            ));
            params.push(Box::new(to.to_string()));
        }
        n += 1;
        sql.push_str(&format!(" ORDER BY pinned DESC, id {order} LIMIT ?{n}"));
        params.push(Box::new(limit as i64));
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(
            params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)),
            |row| {
                Ok((
                    row.get::<_, String>(0).unwrap_or_default(),
                    row.get::<_, String>(1).unwrap_or_default(),
                    row.get::<_, bool>(2).unwrap_or(false),
                    row.get::<_, i64>(3).unwrap_or(0),
                ))
            },
        )
        .ok()
        .map(|m| m.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Record a clipboard capture: dedupes by content (a pinned row is bumped
    /// to the top and keeps its pin), then trims unpinned entries beyond
    /// `keep` so the history stays bounded.
    pub fn save_clipboard(&self, text: &str, keep: usize) {
        if text.is_empty() {
            return;
        }
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let _ = conn.execute(
            "DELETE FROM clipboard_entries WHERE text_content = ?1 AND pinned = 0",
            params![text],
        );
        let pinned_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM clipboard_entries WHERE text_content = ?1 AND pinned = 1)",
                params![text],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if pinned_exists {
            let _ = conn.execute(
                "UPDATE clipboard_entries SET created_at = CURRENT_TIMESTAMP WHERE text_content = ?1 AND pinned = 1",
                params![text],
            );
        } else {
            let _ = conn.execute(
                "INSERT INTO clipboard_entries (text_content) VALUES (?1)",
                params![text],
            );
        }
        let _ = conn.execute(
            "DELETE FROM clipboard_entries WHERE pinned = 0 AND id NOT IN (
                SELECT id FROM clipboard_entries WHERE pinned = 0 ORDER BY id DESC LIMIT ?1
            )",
            params![keep as i64],
        );
    }

    /// Flip the pin on every entry with this text; returns the new state.
    pub fn toggle_clipboard_pinned(&self, text: &str) -> bool {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let current: bool = conn
            .query_row(
                "SELECT pinned FROM clipboard_entries WHERE text_content = ?1 ORDER BY id DESC LIMIT 1",
                params![text],
                |row| row.get(0),
            )
            .unwrap_or(false);
        let new_state = !current;
        let _ = conn.execute(
            "UPDATE clipboard_entries SET pinned = ?2 WHERE text_content = ?1",
            params![text, new_state as i32],
        );
        new_state
    }

    /// Clipboard image history: `(path, width, height, created_at, pinned, id)`,
    /// pinned entries first, then newest first. (Test helper; the provider
    /// uses [`Self::load_clipboard_images_filtered`].)
    #[cfg(test)]
    pub fn load_clipboard_images(
        &self,
        limit: usize,
    ) -> Vec<(String, u32, u32, String, bool, i64)> {
        self.load_clipboard_images_filtered(limit, None, None, true)
    }

    /// Clipboard image history with an optional local-date-range filter (see
    /// [`Self::local_date`]) and sort direction.
    pub fn load_clipboard_images_filtered(
        &self,
        limit: usize,
        date_from: Option<&str>,
        date_to: Option<&str>,
        newest_first: bool,
    ) -> Vec<(String, u32, u32, String, bool, i64)> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let order = if newest_first { "DESC" } else { "ASC" };
        let mut sql =
            "SELECT path, width, height, created_at, pinned, id FROM clipboard_images".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut n = 0usize;
        if let Some(from) = date_from {
            n += 1;
            sql.push_str(&format!(
                " WHERE date(datetime(created_at, 'localtime')) >= ?{n}"
            ));
            params.push(Box::new(from.to_string()));
        }
        if let Some(to) = date_to {
            n += 1;
            sql.push_str(&format!(
                " AND date(datetime(created_at, 'localtime')) <= ?{n}"
            ));
            params.push(Box::new(to.to_string()));
        }
        n += 1;
        sql.push_str(&format!(" ORDER BY pinned DESC, id {order} LIMIT ?{n}"));
        params.push(Box::new(limit as i64));
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        stmt.query_map(
            params_from_iter(params.iter().map(|p| p as &dyn rusqlite::ToSql)),
            |row| {
                Ok((
                    row.get::<_, String>(0).unwrap_or_default(),
                    row.get::<_, u32>(1).unwrap_or(0),
                    row.get::<_, u32>(2).unwrap_or(0),
                    row.get::<_, String>(3).unwrap_or_default(),
                    row.get::<_, bool>(4).unwrap_or(false),
                    row.get::<_, i64>(5).unwrap_or(0),
                ))
            },
        )
        .ok()
        .map(|m| m.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// The local calendar date (`YYYY-MM-DD`) `days_offset` days from today,
    /// computed by SQLite so no date library is needed. `0` = today,
    /// `-1` = yesterday, `-7` = a week ago.
    pub fn local_date(&self, days_offset: i64) -> String {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return String::new(),
        };
        let modifier = format!("{days_offset:+} days");
        conn.query_row(
            "SELECT date('now', 'localtime', ?1)",
            params![modifier],
            |row| row.get(0),
        )
        .unwrap_or_default()
    }

    /// Record a clipboard image capture (dedupe by content hash: an unpinned
    /// duplicate is deleted and re-inserted so it bumps to the top, a pinned
    /// row keeps its pin and just refreshes its timestamp), then trim
    /// unpinned entries beyond `keep`, deleting their cached files from disk.
    pub fn save_clipboard_image(
        &self,
        hash: &str,
        path: &str,
        width: u32,
        height: u32,
        keep: usize,
    ) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        let pinned: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM clipboard_images WHERE hash = ?1 AND pinned = 1)",
                params![hash],
                |row| row.get(0),
            )
            .unwrap_or(false);
        let unpinned: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM clipboard_images WHERE hash = ?1 AND pinned = 0)",
                params![hash],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if unpinned {
            let _ = conn.execute(
                "DELETE FROM clipboard_images WHERE hash = ?1 AND pinned = 0",
                params![hash],
            );
            let _ = conn.execute(
                "INSERT INTO clipboard_images (hash, path, width, height) VALUES (?1, ?2, ?3, ?4)",
                params![hash, path, width as i64, height as i64],
            );
        } else if pinned {
            let _ = conn.execute(
                "UPDATE clipboard_images SET created_at = CURRENT_TIMESTAMP WHERE hash = ?1",
                params![hash],
            );
        } else {
            let _ = conn.execute(
                "INSERT INTO clipboard_images (hash, path, width, height) VALUES (?1, ?2, ?3, ?4)",
                params![hash, path, width as i64, height as i64],
            );
        }
        // Trim unpinned entries beyond the cap, removing their files.
        let doomed: Vec<String> = conn
            .prepare(
                "SELECT path FROM clipboard_images WHERE pinned = 0 AND id NOT IN (
                    SELECT id FROM clipboard_images WHERE pinned = 0 ORDER BY id DESC LIMIT ?1
                )",
            )
            .and_then(|mut stmt| {
                let rows = stmt.query_map(params![keep as i64], |row| row.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()
            })
            .unwrap_or_default();
        for p in &doomed {
            let _ = std::fs::remove_file(p);
        }
        let _ = conn.execute(
            "DELETE FROM clipboard_images WHERE pinned = 0 AND id NOT IN (
                SELECT id FROM clipboard_images WHERE pinned = 0 ORDER BY id DESC LIMIT ?1
            )",
            params![keep as i64],
        );
    }

    /// Flip the pin on the image entry with this cached path; returns the new
    /// state.
    pub fn toggle_clipboard_image_pinned(&self, path: &str) -> bool {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let current: bool = conn
            .query_row(
                "SELECT pinned FROM clipboard_images WHERE path = ?1 ORDER BY id DESC LIMIT 1",
                params![path],
                |row| row.get(0),
            )
            .unwrap_or(false);
        let new_state = !current;
        let _ = conn.execute(
            "UPDATE clipboard_images SET pinned = ?2 WHERE path = ?1",
            params![path, new_state as i32],
        );
        new_state
    }

    /// Create a temporary in-memory database for testing.
    #[cfg(test)]
    pub(crate) fn new_in_memory() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "            CREATE TABLE IF NOT EXISTS clipboard_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL DEFAULT 'text',
                text_content TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                pinned INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS clipboard_images (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                hash TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                pinned INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS frecency (
                app_name TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 1,
                last_used DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS file_frecency (
                path TEXT PRIMARY KEY,
                count INTEGER NOT NULL DEFAULT 1,
                last_used DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS emoji_frecency (
                emoji TEXT PRIMARY KEY,
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

    /// Case-normalized key for the file frecency table — Windows paths are
    /// case-insensitive, so `C:\Reports\Q3` and `c:\reports\q3` must collide.
    fn file_key(path: &str) -> String {
        path.to_lowercase()
    }

    pub fn record_file_open(&self, path: &str) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        conn.execute(
            "INSERT INTO file_frecency (path, count, last_used) VALUES (?1, 1, CURRENT_TIMESTAMP)
             ON CONFLICT(path) DO UPDATE SET count = count + 1, last_used = CURRENT_TIMESTAMP",
            params![Self::file_key(path)],
        )
        .ok();
    }

    pub fn file_frecency_score(&self, path: &str) -> f64 {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return 0.0,
        };
        conn.query_row(
            "SELECT count * (1.0 / (julianday('now') - julianday(last_used) + 1)) FROM file_frecency WHERE path = ?1",
            params![Self::file_key(path)],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
    }

    pub fn record_emoji_use(&self, emoji: &str) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        conn.execute(
            "INSERT INTO emoji_frecency (emoji, count, last_used) VALUES (?1, 1, CURRENT_TIMESTAMP)
             ON CONFLICT(emoji) DO UPDATE SET count = count + 1, last_used = CURRENT_TIMESTAMP",
            params![emoji],
        )
        .ok();
    }

    pub fn emoji_frecency_score(&self, emoji: &str) -> f64 {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return 0.0,
        };
        conn.query_row(
            "SELECT count * (1.0 / (julianday('now') - julianday(last_used) + 1)) FROM emoji_frecency WHERE emoji = ?1",
            params![emoji],
            |row| row.get::<_, f64>(0),
        )
        .unwrap_or(0.0)
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
        // Nothing is pinned by default
        assert!(entries.iter().all(|(_, _, pinned, _)| !pinned));
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
    fn save_clipboard_dedupes_and_bumps() {
        let db = Database::new_in_memory();
        db.save_clipboard("hello", 100);
        db.save_clipboard("world", 100);
        db.save_clipboard("hello", 100);
        let entries = db.load_clipboard(10);
        assert_eq!(entries.len(), 2, "duplicate text stored once");
        assert_eq!(entries[0].0, "hello", "re-copied text bumps to top");
    }

    #[test]
    fn save_clipboard_trims_to_keep() {
        let db = Database::new_in_memory();
        for i in 0..10 {
            db.save_clipboard(&format!("item {i}"), 4);
        }
        let entries = db.load_clipboard(100);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].0, "item 9", "newest kept");
    }

    #[test]
    fn pinned_entries_survive_trim_and_sort_first() {
        let db = Database::new_in_memory();
        for i in 0..10 {
            db.save_clipboard(&format!("item {i}"), 3);
        }
        // Only items 7, 8, 9 survive the trim. Pin the oldest survivor.
        assert!(db.toggle_clipboard_pinned("item 7"));
        for i in 10..13 {
            db.save_clipboard(&format!("item {i}"), 3);
        }
        let entries = db.load_clipboard(100);
        assert_eq!(entries[0].0, "item 7", "pinned entry first");
        assert!(entries[0].2, "pinned flag set");
        assert!(entries.iter().any(|(t, _, _, _)| t == "item 7"));
    }

    #[test]
    fn pinned_row_survives_resave() {
        let db = Database::new_in_memory();
        db.save_clipboard("keep me", 10);
        assert!(db.toggle_clipboard_pinned("keep me"));
        db.save_clipboard("keep me", 10);
        db.save_clipboard("keep me", 10);
        let entries = db.load_clipboard(10);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].2, "pin preserved across re-saves");
    }

    #[test]
    fn image_history_saves_loads_and_dedupes_by_hash() {
        let db = Database::new_in_memory();
        db.save_clipboard_image("hash-a", "C:\\cache\\a.png", 100, 50, 10);
        db.save_clipboard_image("hash-b", "C:\\cache\\b.png", 200, 100, 10);
        db.save_clipboard_image("hash-a", "C:\\cache\\a.png", 100, 50, 10);
        let images = db.load_clipboard_images(10);
        assert_eq!(images.len(), 2, "duplicate hash stored once");
        assert_eq!(
            images[0].0, "C:\\cache\\a.png",
            "re-captured image bumps to top"
        );
        assert_eq!((images[0].1, images[0].2), (100, 50));
    }

    #[test]
    fn image_history_trims_to_keep_and_pins_survive() {
        let db = Database::new_in_memory();
        for i in 0..10 {
            db.save_clipboard_image(
                &format!("hash-{i}"),
                &format!("C:\\cache\\img-{i}.png"),
                10,
                10,
                3,
            );
        }
        // Pin the oldest survivor (7), then push past the cap again.
        assert!(db.toggle_clipboard_image_pinned("C:\\cache\\img-7.png"));
        for i in 10..13 {
            db.save_clipboard_image(
                &format!("hash-{i}"),
                &format!("C:\\cache\\img-{i}.png"),
                10,
                10,
                3,
            );
        }
        let images = db.load_clipboard_images(100);
        assert_eq!(images[0].0, "C:\\cache\\img-7.png", "pinned image first");
        assert!(images[0].4, "pinned flag set");
        assert_eq!(images.len(), 4, "3 unpinned + 1 pinned");
    }

    #[test]
    fn emoji_frecency_starts_at_zero_and_increments() {
        let db = Database::new_in_memory();
        assert_eq!(db.emoji_frecency_score("🔥"), 0.0);
        db.record_emoji_use("🔥");
        let s1 = db.emoji_frecency_score("🔥");
        assert!(s1 > 0.0);
        db.record_emoji_use("🔥");
        assert!(db.emoji_frecency_score("🔥") > s1);
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

    #[test]
    fn file_frecency_starts_at_zero() {
        let db = Database::new_in_memory();
        assert_eq!(db.file_frecency_score(r"C:\Users\me\report.txt"), 0.0);
    }

    #[test]
    fn file_frecency_records_and_increments() {
        let db = Database::new_in_memory();
        db.record_file_open(r"C:\Users\me\report.txt");
        let s1 = db.file_frecency_score(r"C:\Users\me\report.txt");
        assert!(s1 > 0.0);

        db.record_file_open(r"C:\Users\me\report.txt");
        let s2 = db.file_frecency_score(r"C:\Users\me\report.txt");
        assert!(s2 > s1, "second open should increase score");
    }

    #[test]
    fn file_frecency_key_is_case_insensitive() {
        let db = Database::new_in_memory();
        db.record_file_open(r"C:\Users\me\Report.txt");
        assert!(db.file_frecency_score(r"c:\USERS\me\report.txt") > 0.0);
    }
}
