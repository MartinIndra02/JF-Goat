use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum JfgoatError {
    Database(String),
    Http(String),
    Auth(String),
    #[allow(dead_code)]
    NotFound(String),
    Internal(String),
}

pub fn sanitize_error_message(msg: &str) -> String {
    let mut result = String::new();
    let mut current = msg;

    while let Some(pos) = current.find("api_key=") {
        result.push_str(&current[..pos + 8]);
        let remainder = &current[pos + 8..];
        let end_idx = remainder.find(|c| c == '&' || c == ' ' || c == '"' || c == '\'' || c == '|').unwrap_or(remainder.len());
        result.push_str("[REDACTED]");
        current = &remainder[end_idx..];
    }
    result.push_str(current);

    let current_ptr = result;
    result = String::new();
    let mut ptr = current_ptr.as_str();
    while let Some(pos) = ptr.find("Token=\"") {
        result.push_str(&ptr[..pos + 7]);
        let remainder = &ptr[pos + 7..];
        let end_idx = remainder.find('"').unwrap_or(remainder.len());
        result.push_str("[REDACTED]");
        ptr = &remainder[end_idx..];
    }
    result.push_str(ptr);

    let current_ptr2 = result;
    result = String::new();
    let mut ptr2 = current_ptr2.as_str();
    while let Some(pos) = ptr2.find("Token=") {
        result.push_str(&ptr2[..pos + 6]);
        let remainder = &ptr2[pos + 6..];
        let end_idx = remainder.find(|c| c == ' ' || c == ',' || c == '"' || c == '\'' || c == '|').unwrap_or(remainder.len());
        result.push_str("[REDACTED]");
        ptr2 = &remainder[end_idx..];
    }
    result.push_str(ptr2);

    result
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
        use std::error::Error;
        let mut msg = err.to_string();
        let mut source = err.source();
        while let Some(cause) = source {
            msg.push_str(&format!(" | caused by: {}", cause));
            source = cause.source();
        }
        JfgoatError::Http(sanitize_error_message(&msg))
    }
}
