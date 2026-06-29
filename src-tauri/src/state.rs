use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use parking_lot::{RwLock, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;

use crate::db::{
    default_sqlite_read_pool_size,
    SQLITE_BUSY_TIMEOUT_MS,
    SQLITE_WRITE_POOL_SIZE,
};
use crate::error::JfgoatError;
use crate::api::client::JellyfinClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    InitialSync,
    Ready,
}

pub type SqlitePooledConnection = PooledConnection<SqliteConnectionManager>;

#[derive(Clone)]
pub struct DbPool {
    write_pool: Pool<SqliteConnectionManager>,
    read_pool: Pool<SqliteConnectionManager>,
}

impl DbPool {
    pub fn new(db_path: &Path) -> Result<Self, JfgoatError> {
        let read_pool_size = default_sqlite_read_pool_size();

        let write_pool = build_pool(db_path, SQLITE_WRITE_POOL_SIZE, SQLITE_WRITE_POOL_SIZE)?;
        let read_min_idle = (read_pool_size / 2).max(1);
        let read_pool = build_pool(db_path, read_pool_size, read_min_idle)?;

        Ok(Self {
            write_pool,
            read_pool,
        })
    }

    pub fn write_conn(&self) -> Result<SqlitePooledConnection, JfgoatError> {
        self.write_pool
            .get()
            .map_err(|e| JfgoatError::Database(format!("Failed to get SQLite write connection: {e}")))
    }

    pub fn read_conn(&self) -> Result<SqlitePooledConnection, JfgoatError> {
        self.read_pool
            .get()
            .map_err(|e| JfgoatError::Database(format!("Failed to get SQLite read connection: {e}")))
    }

    pub fn lock(&self) -> Result<SqlitePooledConnection, JfgoatError> {
        self.write_conn()
    }
}

fn build_pool(
    db_path: &Path,
    max_size: u32,
    min_idle: u32,
) -> Result<Pool<SqliteConnectionManager>, JfgoatError> {
    Pool::builder()
        .max_size(max_size)
        .min_idle(Some(min_idle))
        .connection_timeout(Duration::from_secs(8))
        .build(sqlite_connection_manager(db_path))
        .map_err(|e| JfgoatError::Database(format!("Failed to build SQLite pool: {e}")))
}

fn sqlite_connection_manager(db_path: &Path) -> SqliteConnectionManager {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_URI;

    SqliteConnectionManager::file(db_path)
        .with_flags(flags)
        // Apply WAL and timeout settings on every pooled connection.
        .with_init(|conn| configure_connection(conn))
}

fn configure_connection(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = MEMORY;",
    )?;
    let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    conn.busy_timeout(Duration::from_millis(SQLITE_BUSY_TIMEOUT_MS))?;
    Ok(())
}

pub struct AppState {
    pub db: DbPool,
    pub http_client: reqwest::Client,
    pub server_url: Arc<RwLock<Option<String>>>,
    pub user_id: Arc<RwLock<Option<String>>>,
    pub token: Arc<RwLock<Option<String>>>,
    pub sync_status: RwLock<SyncStatus>,
    pub user_data_refresh_running: AtomicBool,
    pub sync_running: AtomicBool,
    pub download_trigger: tokio::sync::mpsc::UnboundedSender<()>,
    pub login_attempts: Mutex<HashMap<String, (u32, Instant)>>,
}

impl AppState {
    pub fn get_jf_client(&self) -> Result<(JellyfinClient, String, String), JfgoatError> {
        let (server_url, token, user_id, device_id) = self.get_connection_params()?;
        let server_id = self.get_server_id()?;
        let client = JellyfinClient::new(&self.http_client, &server_url, &device_id).with_token(&token);
        Ok((client, user_id, server_id))
    }

    pub fn get_connection_params(&self) -> Result<(String, String, String, String), JfgoatError> {
        let server_url = self
            .server_url
            .read()
            .clone()
            .ok_or_else(|| JfgoatError::Auth("No server connected".to_string()))?;
        let token = self
            .token
            .read()
            .clone()
            .ok_or_else(|| JfgoatError::Auth("No token".to_string()))?;
        let user_id = self
            .user_id
            .read()
            .clone()
            .ok_or_else(|| JfgoatError::Auth("No user ID".to_string()))?;
        let device_id: String = {
            let db = self
                .db
                .read_conn()
                .map_err(|e| JfgoatError::Internal(e.to_string()))?;
            db.query_row(
                "SELECT value FROM metadata WHERE key = 'device_id'",
                [],
                |row| row.get(0),
            )?
        };
        Ok((server_url, token, user_id, device_id))
    }

    pub fn get_server_id(&self) -> Result<String, JfgoatError> {
        let db = self
            .db
            .read_conn()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;
        let sid: String = db.query_row(
            "SELECT id FROM servers WHERE is_active = 1 ORDER BY connected_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )?;
        Ok(sid)
    }

    pub fn update_session(&self, token: Option<String>, server_url: Option<String>, user_id: Option<String>) {
        *self.token.write() = token;
        *self.server_url.write() = server_url;
        *self.user_id.write() = user_id;
    }
}
