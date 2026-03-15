use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    InitialSync,
    Ready,
}

pub struct AppState {
    pub db: Mutex<Connection>,
    pub http_client: reqwest::Client,
    pub server_url: RwLock<Option<String>>,
    pub user_id: RwLock<Option<String>>,
    pub token: RwLock<Option<String>>,
    pub sync_status: RwLock<SyncStatus>,
}
