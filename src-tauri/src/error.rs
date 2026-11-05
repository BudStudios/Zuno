use serde::{Serialize, Serializer};

/// Application-wide error type. Serialised to a plain message when returned to the UI so that
/// internal details (paths, SQL) are kept to a single human-readable line.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("This action is not permitted")]
    Forbidden,
    #[error("Not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("File error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Window error: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("Invalid address: {0}")]
    Url(#[from] url::ParseError),
    #[error("Data error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Network error: {0}")]
    Http(#[from] reqwest::Error),
    #[cfg(windows)]
    #[error("WebView2 error: {0}")]
    Windows(#[from] windows::core::Error),
}

impl Error {
    pub fn msg(message: impl Into<String>) -> Self {
        Error::Message(message.into())
    }
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
