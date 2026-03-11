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

fn get_device_id(state: &AppState) -> Result<String, JfgoatError> {
    let db = state.db.lock().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let device_id: String = db.query_row(
        "SELECT value FROM metadata WHERE key = 'device_id'",
        [],
        |row| row.get(0),
    )?;
    Ok(device_id)
}

/// Search items - dynamically routes between local FTS5 and remote Jellyfin API
/// based on the current sync status (per SYNC_ARCHITECTURE.md Section 4).
#[tauri::command]
pub async fn search_items(
    query: String,
    state: State<'_, AppState>,
) -> Result<SearchResult, JfgoatError> {
    let sync_status = {
        let status = state.sync_status.read().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *status
    };

    match sync_status {
        SyncStatus::InitialSync => {
            // Bypass SQLite, query remote Jellyfin API directly
            let server_url = {
                let url = state.server_url.read().map_err(|e| JfgoatError::Internal(e.to_string()))?;
                url.clone().ok_or_else(|| JfgoatError::Auth("No server connected".to_string()))?
            };
            let token = {
                let t = state.token.read().map_err(|e| JfgoatError::Internal(e.to_string()))?;
                t.clone().ok_or_else(|| JfgoatError::Auth("No token".to_string()))?
            };
            let user_id = {
                let uid = state.user_id.read().map_err(|e| JfgoatError::Internal(e.to_string()))?;
                uid.clone().ok_or_else(|| JfgoatError::Auth("No user ID".to_string()))?
            };
            let device_id = get_device_id(&state)?;

            let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
                .with_token(&token);

            let response = media_api::search_remote(&jf_client, &user_id, &query, 50).await?;

            // Get server_id for items
            let server_id = {
                let db = state.db.lock().map_err(|e| JfgoatError::Internal(e.to_string()))?;
                let sid: String = db.query_row(
                    "SELECT id FROM servers WHERE is_active = 1 ORDER BY connected_at DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )?;
                sid
            };

            // Convert Jellyfin items to our MediaItem format
            let items: Vec<MediaItem> = response
                .items
                .into_iter()
                .map(|item| {
                    let image_tag = item.image_tags.and_then(|t| t.primary);
                    let backdrop_tag = item.backdrop_image_tags.and_then(|v| v.into_iter().next());
                    let genres = item.genres.map(|g| g.join(", "));
                    let (played, play_count, playback_ticks, is_favorite) = match item.user_data {
                        Some(ud) => (
                            ud.played.unwrap_or(false),
                            ud.play_count.unwrap_or(0),
                            ud.playback_position_ticks.unwrap_or(0),
                            ud.is_favorite.unwrap_or(false),
                        ),
                        None => (false, 0, 0, false),
                    };
                    let name = item.name
                        .filter(|n| !n.trim().is_empty())
                        .unwrap_or_else(|| format!("[{}]", &item.id));

                    MediaItem {
                        id: item.id,
                        name,
                        item_type: item.item_type,
                        parent_id: item.parent_id,
                        series_id: item.series_id,
                        series_name: item.series_name,
                        season_id: item.season_id,
                        season_name: item.season_name,
                        index_number: item.index_number,
                        production_year: item.production_year,
                        overview: item.overview,
                        image_tag,
                        backdrop_tag,
                        date_created: item.date_created,
                        date_updated: item.date_updated,
                        community_rating: item.community_rating,
                        official_rating: item.official_rating,
                        genres,
                        run_time_ticks: item.run_time_ticks,
                        played,
                        play_count,
                        playback_ticks,
                        is_favorite,
                        server_id: server_id.clone(),
                    }
                })
                .collect();

            Ok(SearchResult {
                items,
                source: "remote".to_string(),
            })
        }
        SyncStatus::Ready => {
            // Query local SQLite FTS5 index (sub-millisecond)
            let db = state.db.lock().map_err(|e| JfgoatError::Internal(e.to_string()))?;
            let items = media_db::search_local(&db, &query, 50)?;

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
    let status = state.sync_status.read().map_err(|e| JfgoatError::Internal(e.to_string()))?;
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
