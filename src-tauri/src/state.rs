use rusqlite::Connection;
use std::sync::{Mutex, RwLock};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub http_client: reqwest::Client,
    pub server_url: RwLock<Option<String>>,
    pub user_id: RwLock<Option<String>>,
    pub token: RwLock<Option<String>>,
}
