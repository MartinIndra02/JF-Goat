use serde::Serialize;
use tauri::State;

use crate::api::client::JellyfinClient;
use crate::api::media as media_api;
use crate::db::media::{self as media_db, MediaItem};
use crate::error::JfgoatError;
use crate::state::{AppState, SyncStatus};
use crate::sync;

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub items: Vec<MediaItem>,
    pub source: String, // "local" or "remote"
}

fn get_active_scope(state: &AppState) -> Result<(String, String), JfgoatError> {
    let user_id = state
        .user_id
        .read()
        .clone()
        .ok_or_else(|| JfgoatError::Auth("No user ID".to_string()))?;
    let server_id = state.get_server_id()?;
    Ok((server_id, user_id))
}

/// Search items - dynamically routes between local FTS5 and remote Jellyfin API
/// based on the current sync status (per SYNC_ARCHITECTURE.md Section 4).
#[tauri::command]
pub async fn search_items(
    query: String,
    state: State<'_, AppState>,
) -> Result<SearchResult, JfgoatError> {
    let sync_status = *state.sync_status.read();

    match sync_status {
        SyncStatus::InitialSync => {
            // Bypass SQLite, query remote Jellyfin API directly
            let (server_url, token, user_id, device_id) = state.get_connection_params()?;

            let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
                .with_token(&token);

            let response = media_api::search_remote(&jf_client, &user_id, &query, 50).await?;

            // Get server_id for items
            let server_id = state.get_server_id()?;

            // Convert Jellyfin items to our MediaItem format using standard mapper
            let items: Vec<MediaItem> = response
                .items
                .into_iter()
                .map(|item| MediaItem::from_jellyfin_item(item, &server_id, &user_id))
                .collect();

            Ok(SearchResult {
                items,
                source: "remote".to_string(),
            })
        }
        SyncStatus::Ready => {
            // Query local SQLite FTS5 index (sub-millisecond)
            let db = state.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
            let (server_id, user_id) = get_active_scope(&state)?;
            let items = media_db::search_local(&db, &query, &server_id, &user_id, 50)?;

            Ok(SearchResult {
                items,
                source: "local".to_string(),
            })
        }
    }
}

/// Get the current sync status.
#[tauri::command]
pub async fn get_sync_status(
    state: State<'_, AppState>,
) -> Result<String, JfgoatError> {
    let status = state.sync_status.read();
    match *status {
        SyncStatus::InitialSync => Ok("initial_sync".to_string()),
        SyncStatus::Ready => Ok("ready".to_string()),
    }
}

/// Trigger the background sync. Called by frontend after login or auth check succeeds.
#[tauri::command]
pub async fn start_sync(
    app: tauri::AppHandle,
) -> Result<(), JfgoatError> {
    sync::start_background_sync(app);
    Ok(())
}

/// Force a full re-sync by clearing all checkpoints and media data, then starting sync.
#[tauri::command]
pub async fn force_resync(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), JfgoatError> {
    // Only allow force resync when not already syncing
    {
        let status = state.sync_status.read();
        if *status == SyncStatus::InitialSync {
            return Err(JfgoatError::Internal("Sync already in progress".to_string()));
        }
    }

    // Clear checkpoints and media items
    {
        let (server_id, user_id) = get_active_scope(&state)?;
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        media_db::clear_all_checkpoints(&db, &server_id, &user_id)?;
        db.execute(
            "DELETE FROM media_items WHERE server_id = ?1 AND user_id = ?2",
            rusqlite::params![server_id, user_id],
        )
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    }

    sync::start_background_sync(app);
    Ok(())
}
