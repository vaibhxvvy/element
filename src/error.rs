#![allow(dead_code)]
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ElementError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Icon extraction failed for {path}: {detail}")]
    Icon { path: String, detail: String },

    #[error("Search provider '{provider}' failed: {detail}")]
    Provider { provider: String, detail: String },

    #[error("{0}")]
    Other(String),
}
