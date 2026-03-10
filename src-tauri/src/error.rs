use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum JfgoatError {
    Database(String),
    Http(String),
    Auth(String),
    NotFound(String),
    Internal(String),
}

impl std::error::Error for JfgoatError {}

impl fmt::Display for JfgoatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JfgoatError::Database(msg) => write!(f, "Database error: {}", msg),
            JfgoatError::Http(msg) => write!(f, "HTTP error: {}", msg),
            JfgoatError::Auth(msg) => write!(f, "Auth error: {}", msg),
            JfgoatError::NotFound(msg) => write!(f, "Not found: {}", msg),
            JfgoatError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl From<rusqlite::Error> for JfgoatError {
    fn from(err: rusqlite::Error) -> Self {
        JfgoatError::Database(err.to_string())
    }
}

impl From<reqwest::Error> for JfgoatError {
    fn from(err: reqwest::Error) -> Self {
        JfgoatError::Http(err.to_string())
    }
}
