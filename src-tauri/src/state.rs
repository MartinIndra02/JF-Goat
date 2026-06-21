use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::RwLock;
use std::time::Duration;

use crate::db::{
    default_sqlite_read_pool_size,
    SQLITE_BUSY_TIMEOUT_MS,
    SQLITE_WRITE_POOL_SIZE,
};
use crate::error::JfgoatError;

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
    pub server_url: RwLock<Option<String>>,
    pub user_id: RwLock<Option<String>>,
    pub token: RwLock<Option<String>>,
    pub sync_status: RwLock<SyncStatus>,
    pub user_data_refresh_running: AtomicBool,
}
