use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;

struct DebugLogger {
    file: Mutex<std::fs::File>,
}

impl DebugLogger {
    fn new() -> Option<Self> {
        let mut path = config::data_dir();
        path.push("debug.log");
        if let Some(parent) = path.parent() {
            let _ = create_dir_all(parent);
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok()?;
        Some(Self {
            file: Mutex::new(file),
        })
    }

    fn write(&self, msg: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "[{now}] {msg}");
            let _ = f.flush();
        }
    }
}

static LOGGER: std::sync::OnceLock<DebugLogger> = std::sync::OnceLock::new();

pub fn init() {
    let initialised = LOGGER.set(match DebugLogger::new() {
        Some(l) => l,
        None => return,
    });
    if initialised.is_ok() {
        write("debug logger initialized");
    }
}

fn write(msg: &str) {
    if let Some(logger) = LOGGER.get() {
        logger.write(msg);
    }
    eprintln!("[element-debug] {msg}");
}

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::debug_log::log(&msg);
    }};
}

pub fn log(msg: &str) {
    write(msg);
}
