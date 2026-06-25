use std::sync::atomic::{Ordering};
use tauri::{AppHandle, Manager, Emitter};

pub mod types;
pub mod engine;
pub mod worker;

#[allow(unused_imports)]
pub use types::{SyncProgress, SyncError};

use crate::state::{AppState, SyncStatus};
use engine::run_sync;

// Route legacy print-style sync logs into structured tracing without touching every call site.
macro_rules! println {
    ($($arg:tt)*) => {
        tracing::info!(target: "sync", "{}", format_args!($($arg)*))
    };
}

macro_rules! eprintln {
    ($($arg:tt)*) => {
        tracing::error!(target: "sync", "{}", format_args!($($arg)*))
    };
}

/// Spawn the background indexing worker. Call this after successful authentication.
/// Returns false if a sync is already in progress.
pub fn start_background_sync(app: AppHandle) -> bool {
    let state = match app.try_state::<AppState>() {
        Some(s) => s,
        None => return false,
    };

    // Guard: don't spawn a second sync if one is already running
    if state
        .sync_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        println!("Sync already in progress, skipping duplicate start");
        return false;
    }

    // Determine sync status: only set to InitialSync if we haven't completed all checkpoints
    let all_completed = if let (Ok(server_id), Some(user_id)) = (state.get_server_id(), state.user_id.read().as_ref()) {
        if let Ok(db) = state.db.read_conn() {
            let incomplete_count: u32 = db.query_row(
                "SELECT count(*) FROM sync_checkpoints WHERE status != 'COMPLETED' AND server_id = ?1 AND user_id = ?2",
                rusqlite::params![server_id, user_id],
                |row| row.get(0),
            ).unwrap_or(0);
            let total_checkpoints: u32 = db.query_row(
                "SELECT count(*) FROM sync_checkpoints WHERE server_id = ?1 AND user_id = ?2",
                rusqlite::params![server_id, user_id],
                |row| row.get(0),
            ).unwrap_or(0);
            total_checkpoints > 0 && incomplete_count == 0
        } else {
            false
        }
    } else {
        false
    };

    if !all_completed {
        *state.sync_status.write() = SyncStatus::InitialSync;
    }

    let app_handle = app.clone();
    tokio::spawn(async move {
        let res = run_sync(&app_handle).await;

        if let Some(state) = app_handle.try_state::<AppState>() {
            state.sync_running.store(false, Ordering::SeqCst);
        }

        if let Err(e) = res {
            eprintln!("Background sync failed: {}", e);
            // Ensure we never leave the status stuck at InitialSync
            if let Some(state) = app_handle.try_state::<AppState>() {
                *state.sync_status.write() = SyncStatus::Ready;
            }
            let _ = app_handle.emit("sync-error", SyncError {
                message: e.to_string(),
                batch_start: 0,
                batch_size: 0,
                retries_attempted: 0,
                is_fatal: true,
            });
        }
    });

    true
}
