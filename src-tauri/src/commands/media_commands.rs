use serde::{Deserialize, Serialize};
use std::fs;
use tauri::{Manager, State};

use crate::api::client::JellyfinClient;
use crate::api::media as media_api;
use crate::db::media::{to_paginated_result, MediaItem, PaginationScope};
use crate::error::JfgoatError;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackLifecycleEvent {
    Playing,
    Progress,
    Stopped,
}

impl PlaybackLifecycleEvent {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "playing" => Some(Self::Playing),
            "progress" => Some(Self::Progress),
            "stopped" => Some(Self::Stopped),
            _ => None,
        }
    }
}

/// A person (actor, director, etc.) associated with a media item.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    pub person_type: Option<String>,
    pub image_tag: Option<String>,
}

fn row_to_media_item(row: &rusqlite::Row) -> rusqlite::Result<MediaItem> {
    Ok(MediaItem {
        id: row.get(0)?,
        name: row.get(1)?,
        item_type: row.get(2)?,
        parent_id: row.get(3)?,
        series_id: row.get(4)?,
        series_name: row.get(5)?,
        season_id: row.get(6)?,
        season_name: row.get(7)?,
        index_number: row.get(8)?,
        production_year: row.get(9)?,
        overview: row.get(10)?,
        image_tag: row.get(11)?,
        backdrop_tag: row.get(12)?,
        date_created: row.get(13)?,
        date_updated: row.get(14)?,
        community_rating: row.get(15)?,
        official_rating: row.get(16)?,
        genres: row.get(17)?,
        run_time_ticks: row.get(18)?,
        played: row.get::<_, i32>(19)? != 0,
        play_count: row.get(20)?,
        playback_ticks: row.get(21)?,
        is_favorite: row.get::<_, i32>(22)? != 0,
        server_id: row.get(23)?,
        user_id: row.get(24)?,
    })
}

fn is_db_lock_contention(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(inner, _)
            if matches!(
                inner.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            )
    )
}

const SELECT_COLUMNS: &str = "id, name, type, parent_id, series_id, series_name,
     season_id, season_name, index_number, production_year,
     overview, image_tag, backdrop_tag, date_created, date_updated,
     community_rating, official_rating, genres, run_time_ticks,
    played, play_count, playback_ticks, is_favorite, server_id, user_id";

/// Convert a Jellyfin API item into our local MediaItem format.
fn jf_item_to_media_item(
    item: media_api::JellyfinItem,
    server_id: &str,
    user_id: &str,
) -> MediaItem {
    MediaItem::from_jellyfin_item(item, server_id, user_id)
}

/// Read connection parameters from AppState.
fn get_connection_params(state: &AppState) -> Result<(String, String, String, String), JfgoatError> {
    state.get_connection_params()
}

fn get_server_id(state: &AppState) -> Result<String, JfgoatError> {
    state.get_server_id()
}

fn get_active_scope(state: &AppState) -> Result<(String, String), JfgoatError> {
    let server_id = state.get_server_id()?;
    let user_id = state
        .user_id
        .read()
        .clone()
        .ok_or_else(|| JfgoatError::Auth("No user ID".to_string()))?;

    Ok((server_id, user_id))
}

fn query_local_library_items_by_parent(
    state: &AppState,
    parent_id: &str,
    server_id: &str,
    user_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<media_api::PaginatedResult<MediaItem>, JfgoatError> {
    let db = state
        .db
        .read_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let total_count: u32 = db.query_row(
        "SELECT COUNT(*) FROM media_items WHERE parent_id = ?1 AND server_id = ?2 AND user_id = ?3",
        rusqlite::params![parent_id, server_id, user_id],
        |row| row.get(0),
    )?;

    let sql = format!(
        "SELECT {} FROM media_items
         WHERE parent_id = ?1 AND server_id = ?2 AND user_id = ?3
         ORDER BY COALESCE(date_updated, date_created) DESC, name ASC
         LIMIT ?4 OFFSET ?5",
        SELECT_COLUMNS
    );

    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params![
            parent_id,
            server_id,
            user_id,
            limit as i64,
            start_index as i64
        ],
        row_to_media_item,
    )?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }

    Ok(to_paginated_result(
        items,
        PaginationScope { start_index, limit },
        Some(total_count),
    ))
}

fn query_local_library_items_by_server_type(
    state: &AppState,
    server_id: &str,
    user_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<media_api::PaginatedResult<MediaItem>, JfgoatError> {
    let db = state
        .db
        .read_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let total_count: u32 = db.query_row(
        "SELECT COUNT(*) FROM media_items
         WHERE server_id = ?1
           AND user_id = ?2
           AND type IN ('Movie', 'Series', 'Season')",
        rusqlite::params![server_id, user_id],
        |row| row.get(0),
    )?;

    let sql = format!(
        "SELECT {} FROM media_items
         WHERE server_id = ?1
           AND user_id = ?2
           AND type IN ('Movie', 'Series', 'Season')
         ORDER BY COALESCE(date_updated, date_created) DESC, name ASC
         LIMIT ?3 OFFSET ?4",
        SELECT_COLUMNS
    );

    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params![server_id, user_id, limit as i64, start_index as i64],
        row_to_media_item,
    )?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }

    Ok(to_paginated_result(
        items,
        PaginationScope { start_index, limit },
        Some(total_count),
    ))
}

fn get_library_items_from_local_fallback(
    state: &AppState,
    parent_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<media_api::PaginatedResult<MediaItem>, JfgoatError> {
    let (server_id, user_id) = get_active_scope(state)?;

    let by_parent = query_local_library_items_by_parent(
        state,
        parent_id,
        &server_id,
        &user_id,
        start_index,
        limit,
    )?;

    if by_parent.total_record_count > 0 {
        return Ok(by_parent);
    }

    query_local_library_items_by_server_type(state, &server_id, &user_id, start_index, limit)
}

// ── Local DB queries (used as fallback / for search) ────────────────────

#[tauri::command]
pub fn get_recent_movies(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (server_id, user_id) = get_active_scope(&state)?;
    let db = state
        .db
        .read_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items WHERE type = 'Movie' AND server_id = ?1 AND user_id = ?2 ORDER BY date_created DESC LIMIT ?3",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![server_id, user_id, limit], |row| row_to_media_item(row))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

#[tauri::command]
pub fn get_recent_series(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (server_id, user_id) = get_active_scope(&state)?;
    let db = state
        .db
        .read_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items WHERE type = 'Series' AND server_id = ?1 AND user_id = ?2 ORDER BY date_created DESC LIMIT ?3",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![server_id, user_id, limit], |row| row_to_media_item(row))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

#[tauri::command]
pub fn get_continue_watching(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (server_id, user_id) = get_active_scope(&state)?;
    let db = state
        .db
        .read_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items
         WHERE playback_ticks > 0 AND played = 0 AND server_id = ?1 AND user_id = ?2
         ORDER BY date_updated DESC
         LIMIT ?3",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![server_id, user_id, limit], |row| row_to_media_item(row))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

#[tauri::command]
pub fn get_latest_media(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (server_id, user_id) = get_active_scope(&state)?;
    let db = state
        .db
        .read_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items
         WHERE backdrop_tag IS NOT NULL AND type IN ('Movie', 'Series') AND server_id = ?1 AND user_id = ?2
         ORDER BY date_created DESC
         LIMIT ?3",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![server_id, user_id, limit], |row| row_to_media_item(row))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

// ── Live Jellyfin API commands (real-time data from server) ─────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserLibrary {
    pub id: String,
    pub name: String,
    pub collection_type: Option<String>,
}

// ── Homepage cache for instant startup ──────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HomepageCache {
    pub resume_items: Vec<MediaItem>,
    pub next_up_items: Vec<MediaItem>,
    pub user_libraries: Vec<UserLibrary>,
    pub library_latest: std::collections::HashMap<String, Vec<MediaItem>>,
    pub featured_items: Vec<MediaItem>,
    #[serde(default)]
    pub cache_refreshed_at_epoch_ms: Option<u64>,
}

/// Persist the homepage dashboard data to a JSON file for instant startup.
#[tauri::command]
pub async fn save_homepage_cache(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    data: HomepageCache,
) -> Result<(), JfgoatError> {
    let server_id = state.get_server_id().unwrap_or_else(|_| "unknown_server".to_string());
    let user_id = state.user_id.read().clone().unwrap_or_else(|| "unknown_user".to_string());

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let _ = fs::create_dir_all(&cache_dir);
    
    let safe_user_id = user_id.replace(|c: char| !c.is_alphanumeric(), "_");
    let safe_server_id = server_id.replace(|c: char| !c.is_alphanumeric(), "_");
    let cache_path = cache_dir.join(format!("homepage_cache_{}_{}.json", safe_server_id, safe_user_id));

    let json = serde_json::to_string(&data)
        .map_err(|e| JfgoatError::Internal(format!("JSON serialize error: {}", e)))?;

    fs::write(&cache_path, json)
        .map_err(|e| JfgoatError::Internal(format!("Cache write error: {}", e)))?;

    Ok(())
}

/// Load the cached homepage data from disk. Returns null if no cache exists.
#[tauri::command]
pub async fn load_homepage_cache(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<HomepageCache>, JfgoatError> {
    let server_id = state.get_server_id().unwrap_or_else(|_| "unknown_server".to_string());
    let user_id = state.user_id.read().clone().unwrap_or_else(|| "unknown_user".to_string());

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
        
    let safe_user_id = user_id.replace(|c: char| !c.is_alphanumeric(), "_");
    let safe_server_id = server_id.replace(|c: char| !c.is_alphanumeric(), "_");
    let cache_path = cache_dir.join(format!("homepage_cache_{}_{}.json", safe_server_id, safe_user_id));

    if !cache_path.exists() {
        return Ok(None);
    }

    let json = fs::read_to_string(&cache_path)
        .map_err(|e| JfgoatError::Internal(format!("Cache read error: {}", e)))?;

    let data: HomepageCache = serde_json::from_str(&json)
        .map_err(|e| JfgoatError::Internal(format!("Cache parse error: {}", e)))?;

    Ok(Some(data))
}

/// Fetch the user's libraries (Views) directly from the Jellyfin server.
#[tauri::command]
pub async fn get_user_views(
    state: State<'_, AppState>,
) -> Result<Vec<UserLibrary>, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let response = media_api::fetch_user_views(&jf_client, &user_id).await?;

    let views: Vec<UserLibrary> = response
        .items
        .into_iter()
        .map(|v| UserLibrary {
            id: v.id,
            name: v.name.unwrap_or_default(),
            collection_type: v.collection_type,
        })
        .collect();

    Ok(views)
}

/// Fetch resume items (continue watching) directly from the Jellyfin server.
#[tauri::command]
pub async fn get_resume_items(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;
    let server_id = get_server_id(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let response = media_api::fetch_resume_items(&jf_client, &user_id, limit).await?;

    let items: Vec<MediaItem> = response
        .items
        .into_iter()
        .map(|item| jf_item_to_media_item(item, &server_id, &user_id))
        .collect();

    Ok(items)
}

/// Fetch next up episodes directly from the Jellyfin server.
#[tauri::command]
pub async fn get_next_up(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;
    let server_id = get_server_id(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let response = media_api::fetch_next_up(&jf_client, &user_id, limit).await?;

    let items: Vec<MediaItem> = response
        .items
        .into_iter()
        .map(|item| jf_item_to_media_item(item, &server_id, &user_id))
        .collect();

    Ok(items)
}

// ── Detail page queries (local DB → API fallback) ───────────────────────

/// Get a single media item by ID. Tries local DB first; falls back to
/// the live Jellyfin API if the item hasn't been synced yet.
#[tauri::command]
pub async fn get_item_by_id(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<MediaItem>, JfgoatError> {
    let (scope_server_id, scope_user_id) = get_active_scope(&state)?;

    // 1. Try local DB first (fast path)
    {
        let db = state
            .db
            .read_conn()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;

        let sql = format!(
            "SELECT {} FROM media_items WHERE id = ?1 AND server_id = ?2 AND user_id = ?3",
            SELECT_COLUMNS
        );
        let mut stmt = db.prepare(&sql)?;
        let result = stmt.query_row(
            rusqlite::params![id, scope_server_id, scope_user_id],
            |row| row_to_media_item(row),
        );

        match result {
            Ok(item) => return Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => { /* fall through to API */ }
            Err(e) => return Err(e.into()),
        }
    }

    // 2. Fallback: fetch from Jellyfin API
    let params = get_connection_params(&state);
    let server_id = get_server_id(&state);
    if let (Ok((server_url, token, user_id, device_id)), Ok(sid)) = (params, server_id) {
        let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
            .with_token(&token);

        match media_api::fetch_item_by_id(&jf_client, &user_id, &id).await {
            Ok(jf_item) => return Ok(Some(jf_item_to_media_item(jf_item, &sid, &user_id))),
            Err(e) => {
                println!("[detail] API fallback failed for item {}: {}", id, e);
            }
        }
    }

    Ok(None)
}

/// Get seasons for a series. Tries local DB first; falls back to
/// the live Jellyfin API if the seasons haven't been synced yet.
#[tauri::command]
pub async fn get_series_seasons(
    state: State<'_, AppState>,
    series_id: String,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (scope_server_id, scope_user_id) = get_active_scope(&state)?;

    // 1. Try local DB first
    {
        let db = state
            .db
            .read_conn()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;

        let sql = format!(
            "SELECT {} FROM media_items WHERE series_id = ?1 AND type = 'Season' AND server_id = ?2 AND user_id = ?3 ORDER BY index_number ASC",
            SELECT_COLUMNS
        );
        let mut stmt = db.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params![series_id, scope_server_id, scope_user_id],
            |row| row_to_media_item(row),
        )?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        if !items.is_empty() {
            return Ok(items);
        }
    }

    // 2. Fallback: fetch from Jellyfin API
    let params = get_connection_params(&state);
    let server_id = get_server_id(&state);
    if let (Ok((server_url, token, user_id, device_id)), Ok(sid)) = (params, server_id) {
        let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
            .with_token(&token);

        match media_api::fetch_seasons(&jf_client, &user_id, &series_id).await {
            Ok(response) => {
                let items: Vec<MediaItem> = response
                    .items
                    .into_iter()
                    .map(|item| jf_item_to_media_item(item, &sid, &user_id))
                    .collect();
                return Ok(items);
            }
            Err(e) => {
                println!("[detail] API fallback failed for seasons of {}: {}", series_id, e);
            }
        }
    }

    Ok(vec![])
}

/// Get episodes for a season. Tries local DB first; falls back to
/// the live Jellyfin API if the episodes haven't been synced yet.
#[tauri::command]
pub async fn get_season_episodes(
    state: State<'_, AppState>,
    season_id: String,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (scope_server_id, scope_user_id) = get_active_scope(&state)?;

    // 1. Try local DB first
    let series_id_for_fallback: Option<String>;
    {
        let db = state
            .db
            .read_conn()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;

        let sql = format!(
            "SELECT {} FROM media_items WHERE season_id = ?1 AND type = 'Episode' AND server_id = ?2 AND user_id = ?3 ORDER BY index_number ASC",
            SELECT_COLUMNS
        );
        let local_items_result: Result<Vec<MediaItem>, rusqlite::Error> = (|| {
            let mut stmt = db.prepare_cached(&sql)?;
            let rows = stmt.query_map(
                rusqlite::params![&season_id, scope_server_id, scope_user_id],
                row_to_media_item,
            )?;

            let mut items = Vec::new();
            for row in rows {
                items.push(row?);
            }
            Ok(items)
        })();

        match local_items_result {
            Ok(items) => {
                if !items.is_empty() {
                    return Ok(items);
                }
            }
            Err(e) if is_db_lock_contention(&e) => {
                println!(
                    "[detail] Local season episode query hit SQLite contention; falling back to API: {}",
                    e
                );
            }
            Err(e) => return Err(e.into()),
        }

        // For API fallback we need the series_id; try to get it from the season record.
        // During sync writes we tolerate transient lock contention and continue with API fallback.
        series_id_for_fallback = match db.query_row(
            "SELECT series_id FROM media_items WHERE id = ?1 AND server_id = ?2 AND user_id = ?3",
            rusqlite::params![&season_id, scope_server_id, scope_user_id],
            |row| row.get::<_, Option<String>>(0),
        ) {
            Ok(value) => value,
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) if is_db_lock_contention(&e) => {
                println!(
                    "[detail] Local season->series lookup hit SQLite contention; falling back to API: {}",
                    e
                );
                None
            }
            Err(e) => return Err(e.into()),
        };
    }

    // 2. Fallback: fetch from Jellyfin API
    //    If we don't have the series_id from the local DB (season not synced),
    //    fetch the season item from the API first to discover its series_id.
    let mut series_id_resolved = series_id_for_fallback;
    if series_id_resolved.is_none() {
        if let (Ok((ref server_url, ref token, ref user_id, ref device_id)), Ok(ref _sid)) =
            (get_connection_params(&state), get_server_id(&state))
        {
            let jf_client = JellyfinClient::new(&state.http_client, server_url, device_id)
                .with_token(token);
            if let Ok(season_item) = media_api::fetch_item_by_id(&jf_client, user_id, &season_id).await {
                series_id_resolved = season_item.series_id;
            }
        }
    }

    if let Some(ref sid_for_api) = series_id_resolved {
        let params = get_connection_params(&state);
        let server_id = get_server_id(&state);
        if let (Ok((server_url, token, user_id, device_id)), Ok(sid)) = (params, server_id) {
            let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
                .with_token(&token);

            match media_api::fetch_episodes(&jf_client, &user_id, sid_for_api, &season_id).await {
                Ok(response) => {
                    let items: Vec<MediaItem> = response
                        .items
                        .into_iter()
                        .map(|item| jf_item_to_media_item(item, &sid, &user_id))
                        .collect();
                    return Ok(items);
                }
                Err(e) => {
                    println!("[detail] API fallback failed for episodes of season {}: {}", season_id, e);
                }
            }
        }
    }

    Ok(vec![])
}

/// Fetch latest items for a specific library view from the Jellyfin server.
#[tauri::command]
pub async fn get_latest_items(
    state: State<'_, AppState>,
    parent_id: String,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;
    let server_id = get_server_id(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let items_raw =
        media_api::fetch_latest_items(&jf_client, &user_id, &parent_id, limit).await?;

    let items: Vec<MediaItem> = items_raw
        .into_iter()
        .map(|item| jf_item_to_media_item(item, &server_id, &user_id))
        .collect();

    Ok(items)
}

/// Fetch paginated items for a specific library view from the Jellyfin server.
#[tauri::command]
pub async fn get_library_items(
    state: State<'_, AppState>,
    parent_id: String,
    page: u32,
    limit: u32,
    prefer_offline: Option<bool>,
) -> Result<media_api::PaginatedResult<MediaItem>, JfgoatError> {
    let safe_page = page.max(1);
    let safe_limit = limit.clamp(1, 500);
    let start_index = safe_page.saturating_sub(1).saturating_mul(safe_limit);
    let enable_total_count = true;

    if prefer_offline.unwrap_or(false) {
        return get_library_items_from_local_fallback(&state, &parent_id, start_index, safe_limit);
    }

    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;
    let server_id = get_server_id(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let result = media_api::fetch_view_items_paginated(
        &jf_client,
        &user_id,
        &parent_id,
        start_index,
        safe_limit,
        enable_total_count,
    )
    .await;

    let result = match result {
        Ok(result) => result,
        Err(remote_error) => {
            println!(
                "[library] Live fetch failed for view {} page {} (start {}): {}. Falling back to local DB.",
                parent_id, safe_page, start_index, remote_error
            );

            match get_library_items_from_local_fallback(&state, &parent_id, start_index, safe_limit)
            {
                Ok(local_result) => return Ok(local_result),
                Err(local_error) => {
                    println!(
                        "[library] Local fallback failed for view {}: {}",
                        parent_id, local_error
                    );
                    return Err(remote_error);
                }
            }
        }
    };

    let items: Vec<MediaItem> = result
        .items
        .into_iter()
        .map(|item| jf_item_to_media_item(item, &server_id, &user_id))
        .collect();

    Ok(media_api::PaginatedResult {
        items,
        total_record_count: result.total_record_count,
        start_index: result.start_index,
        limit: result.limit,
        has_more: result.has_more,
    })
}

/// Fetch cast & crew (people) for a media item from the Jellyfin server.
#[tauri::command]
pub async fn get_item_people(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<Person>, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let people = media_api::fetch_item_people(&jf_client, &user_id, &id).await?;

    let result: Vec<Person> = people
        .into_iter()
        .map(|p| Person {
            id: p.id,
            name: p.name.unwrap_or_default(),
            role: p.role,
            person_type: p.person_type,
            image_tag: p.primary_image_tag,
        })
        .collect();

    Ok(result)
}

/// Fetch similar/related items for a media item from the Jellyfin server.
#[tauri::command]
pub async fn get_similar_items(
    state: State<'_, AppState>,
    id: String,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;
    let server_id = get_server_id(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let response = media_api::fetch_similar_items(&jf_client, &user_id, &id, limit).await?;

    let items: Vec<MediaItem> = response
        .items
        .into_iter()
        .map(|item| jf_item_to_media_item(item, &server_id, &user_id))
        .collect();

    Ok(items)
}

// ── Media streams and external URLs for detail pages ────────────────────

/// A single stream option (video, audio, or subtitle track).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamOption {
    pub index: i64,
    pub codec: String,
    pub display_title: String,
    pub language: Option<String>,
    pub is_default: bool,
    pub delivery_method: Option<String>,
    pub is_external: bool,
    pub height: Option<i64>,
    pub width: Option<i64>,
    pub bit_rate: Option<i64>,
    pub channels: Option<i64>,
    pub channel_layout: Option<String>,
    pub video_range: Option<String>,
    pub video_range_type: Option<String>,
}

/// Grouped media stream info for an item.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaStreamInfo {
    pub video: Vec<StreamOption>,
    pub audio: Vec<StreamOption>,
    pub subtitle: Vec<StreamOption>,
    /// Short label for the primary video stream, e.g. "HD SDR"
    pub video_label: Option<String>,
}

/// An external URL (e.g. IMDb, TMDB, TheTVDB).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExternalUrl {
    pub name: String,
    pub url: String,
}

/// A single chapter marker in a media item timeline.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChapterInfo {
    pub name: String,
    pub start_ticks: i64,
    pub image_tag: Option<String>,
    pub marker_type: Option<String>,
    pub chapter_type: Option<String>,
}

/// Fetch media stream info (video quality, audio tracks, subtitles) for a media item.
#[tauri::command]
pub async fn get_media_streams(
    state: State<'_, AppState>,
    id: String,
) -> Result<MediaStreamInfo, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let streams = media_api::fetch_item_media_streams(&jf_client, &user_id, &id).await?;

    let mut video = Vec::new();
    let mut audio = Vec::new();
    let mut subtitle = Vec::new();
    let mut video_label: Option<String> = None;

    for s in streams {
        let stream_type = s.stream_type.as_deref().unwrap_or("");
        let option = StreamOption {
            index: s.index.unwrap_or(0),
            codec: s.codec.clone().unwrap_or_default().to_uppercase(),
            display_title: s.display_title.clone().unwrap_or_default(),
            language: s.language.clone(),
            is_default: s.is_default.unwrap_or(false),
            delivery_method: s.delivery_method.clone(),
            is_external: s.is_external.unwrap_or(false),
            height: s.height,
            width: s.width,
            bit_rate: s.bit_rate,
            channels: s.channels,
            channel_layout: s.channel_layout.clone(),
            video_range: s.video_range.clone(),
            video_range_type: s.video_range_type.clone(),
        };

        match stream_type {
            "Video" => {
                // Build a short label like "HD SDR" or "4K HDR"
                if video_label.is_none() {
                    let resolution = match s.height.unwrap_or(0) {
                        h if h >= 2160 => "4K",
                        h if h >= 1080 => "HD",
                        h if h >= 720 => "HD",
                        h if h > 0 => "SD",
                        _ => "HD",
                    };
                    let range = s.video_range.as_deref().unwrap_or("SDR");
                    video_label = Some(format!("{} {}", resolution, range));
                }
                video.push(option);
            }
            "Audio" => audio.push(option),
            "Subtitle" => subtitle.push(option),
            _ => {}
        }
    }

    Ok(MediaStreamInfo {
        video,
        audio,
        subtitle,
        video_label,
    })
}

/// Fetch external URLs (IMDb, TMDB, TheTVDB, etc.) for a media item.
#[tauri::command]
pub async fn get_external_urls(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ExternalUrl>, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let urls = media_api::fetch_item_external_urls(&jf_client, &user_id, &id).await?;

    let result: Vec<ExternalUrl> = urls
        .into_iter()
        .filter_map(|u| {
            let name = u.name?;
            let url = u.url?;
            if url.is_empty() { return None; }
            Some(ExternalUrl { name, url })
        })
        .collect();

    Ok(result)
}

/// Fetch chapter markers for a media item.
#[tauri::command]
pub async fn get_item_chapters(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ChapterInfo>, JfgoatError> {
    let (server_url, token, _user_id, device_id) = get_connection_params(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let chapters = media_api::fetch_item_chapters(&jf_client, &id).await?;

    let result = chapters
        .into_iter()
        .map(|chapter| ChapterInfo {
            name: chapter.name.unwrap_or_else(|| "Chapter".to_string()),
            start_ticks: chapter.start_position_ticks.unwrap_or(0),
            image_tag: chapter.image_tag,
            marker_type: chapter.marker_type,
            chapter_type: chapter.chapter_type,
        })
        .collect();

    Ok(result)
}

// ── User data mutations ─────────────────────────────────────────────────

/// Toggle the played/unplayed state for a media item on the Jellyfin server
/// and update the local DB. Returns the new played state.
#[tauri::command]
pub async fn toggle_played(
    state: State<'_, AppState>,
    id: String,
    played: bool,
) -> Result<bool, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;
    let server_id = get_server_id(&state)?;
    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let new_played = !played;
    if new_played {
        media_api::mark_played(&jf_client, &user_id, &id).await?;
    } else {
        media_api::mark_unplayed(&jf_client, &user_id, &id).await?;
    }

    // Fetch the updated item from the server to get its new UserData
    let jf_item = media_api::fetch_item_by_id(&jf_client, &user_id, &id).await?;
    let item_type = jf_item.item_type.clone();
    let updated_item = jf_item_to_media_item(jf_item, &server_id, &user_id);

    let mut items_to_update = vec![updated_item.clone()];

    if item_type == "Series" {
        if let Ok(children_resp) = media_api::fetch_series_children(&jf_client, &user_id, &id, 0, 500).await {
            for child in children_resp.items {
                items_to_update.push(jf_item_to_media_item(child, &server_id, &user_id));
            }
        }
    } else if item_type == "Season" {
        if let Some(ref series_id) = updated_item.series_id {
            if let Ok(episodes_resp) = media_api::fetch_episodes(&jf_client, &user_id, series_id, &id).await {
                for ep in episodes_resp.items {
                    items_to_update.push(jf_item_to_media_item(ep, &server_id, &user_id));
                }
            }
        }
    }

    // Update local DB in a single transaction
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        crate::db::media::insert_media_chunk(&db, &items_to_update)?;
    }

    Ok(new_played)
}

/// Toggle the favorite state for a media item on the Jellyfin server
/// and update the local DB. Returns the new favorite state.
#[tauri::command]
pub async fn toggle_favorite(
    state: State<'_, AppState>,
    id: String,
    is_favorite: bool,
) -> Result<bool, JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(&state)?;
    let server_id = get_server_id(&state)?;
    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let new_favorite = !is_favorite;
    if new_favorite {
        media_api::mark_favorite(&jf_client, &user_id, &id).await?;
    } else {
        media_api::unmark_favorite(&jf_client, &user_id, &id).await?;
    }

    // Fetch the updated item from the server to get its new UserData
    let jf_item = media_api::fetch_item_by_id(&jf_client, &user_id, &id).await?;
    let updated_item = jf_item_to_media_item(jf_item, &server_id, &user_id);

    // Update local DB fully
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        crate::db::media::insert_media_chunk(&db, &[updated_item])?;
    }

    Ok(new_favorite)
}

pub fn apply_user_data_refresh_batch(
    state: &AppState,
    server_id: &str,
    user_id: &str,
    items: &[media_api::JellyfinItem],
) -> Result<u32, JfgoatError> {
    if items.is_empty() {
        return Ok(0);
    }

    let db = state
        .db
        .write_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let tx = db.unchecked_transaction()?;
    let mut updated = 0u32;

    {
        let mut stmt = tx.prepare_cached(
            "UPDATE media_items
             SET played = ?1,
                 play_count = ?2,
                 playback_ticks = ?3,
                 is_favorite = ?4
             WHERE id = ?5 AND server_id = ?6 AND user_id = ?7",
        )?;

        for item in items {
            let user_data = item.user_data.as_ref();
            let played = user_data.and_then(|d| d.played).unwrap_or(false) as i32;
            let play_count = user_data.and_then(|d| d.play_count).unwrap_or(0);
            let playback_ticks = user_data
                .and_then(|d| d.playback_position_ticks)
                .unwrap_or(0)
                .max(0);
            let is_favorite = user_data
                .and_then(|d| d.is_favorite)
                .unwrap_or(false) as i32;

            let rows = stmt.execute(rusqlite::params![
                played,
                play_count,
                playback_ticks,
                is_favorite,
                item.id,
                server_id,
                user_id,
            ])?;
            updated += rows as u32;
        }
    }

    tx.commit()?;
    Ok(updated)
}

/// Report playback lifecycle events to Jellyfin and keep local playback flags in sync.
pub async fn report_playback_lifecycle_internal(
    state: &AppState,
    item_id: &str,
    position_ticks: i64,
    duration_ticks: i64,
    event: PlaybackLifecycleEvent,
) -> Result<(), JfgoatError> {
    let (server_url, token, user_id, device_id) = get_connection_params(state)?;
    let server_id = get_server_id(state)?;
    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let safe_position = position_ticks.max(0);
    let safe_duration = duration_ticks.max(0);

    let report_result = match event {
        PlaybackLifecycleEvent::Playing => {
            media_api::report_playback_playing(&jf_client, item_id, safe_position).await
        }
        PlaybackLifecycleEvent::Progress => {
            media_api::report_playback_progress(&jf_client, item_id, safe_position).await
        }
        PlaybackLifecycleEvent::Stopped => {
            media_api::report_playback_stopped(&jf_client, item_id, safe_position).await
        }
    };

    report_result?;

    let near_end = if safe_duration > 0 {
        let remaining = (safe_duration - safe_position).max(0);
        let remaining_threshold = 60 * 10_000_000; // 60s
        let percent = safe_position as f64 / safe_duration as f64;
        percent >= 0.90 && (remaining <= remaining_threshold || percent >= 0.95)
    } else {
        false
    };

    if event == PlaybackLifecycleEvent::Stopped && near_end {
        media_api::mark_played(&jf_client, &user_id, &item_id).await?;
    }

    {
        let db = state
            .db
            .write_conn()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;

        if event == PlaybackLifecycleEvent::Stopped && near_end {
            db.execute(
                "UPDATE media_items SET played = 1, playback_ticks = 0 WHERE id = ?1 AND server_id = ?2 AND user_id = ?3",
                rusqlite::params![item_id, server_id, user_id],
            )?;
        } else {
            db.execute(
                "UPDATE media_items SET played = 0, playback_ticks = ?1 WHERE id = ?2 AND server_id = ?3 AND user_id = ?4",
                rusqlite::params![safe_position, item_id, server_id, user_id],
            )?;
        }
    }

    Ok(())
}

/// Report playback stop to Jellyfin and update local playback flags.
#[tauri::command]
pub async fn report_playback_stopped(
    state: State<'_, AppState>,
    item_id: String,
    position_ticks: i64,
    duration_ticks: i64,
) -> Result<(), JfgoatError> {
    report_playback_lifecycle_internal(
        &state,
        &item_id,
        position_ticks,
        duration_ticks,
        PlaybackLifecycleEvent::Stopped,
    )
    .await
}
