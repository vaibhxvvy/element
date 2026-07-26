use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use arboard::Clipboard;

use crate::database::Database;

pub fn start_clipboard_monitor(
    _db: Database,
    tx: mpsc::Sender<String>,
) {
    thread::spawn(move || {
        let db = Database::init();
        let mut clip = match Clipboard::new() {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut last = String::new();

        loop {
            thread::sleep(Duration::from_millis(500));
            if let Ok(text) = clip.get_text() {
                if text != last && !text.is_empty() {
                    last = text.clone();
                    db.store_clipboard(&text);
                    let _ = tx.send(text);
                }
            }
        }
    });
}
