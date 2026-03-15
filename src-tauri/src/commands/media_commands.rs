use serde::{Deserialize, Serialize};
use std::fs;
use tauri::{Manager, State};

use crate::api::client::JellyfinClient;
use crate::api::media as media_api;
use crate::db::media::MediaItem;
use crate::error::JfgoatError;
use crate::state::AppState;

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
    })
}

const SELECT_COLUMNS: &str = "id, name, type, parent_id, series_id, series_name,
     season_id, season_name, index_number, production_year,
     overview, image_tag, backdrop_tag, date_created, date_updated,
     community_rating, official_rating, genres, run_time_ticks,
     played, play_count, playback_ticks, is_favorite, server_id";

/// Convert a Jellyfin API item into our local MediaItem format.
fn jf_item_to_media_item(item: media_api::JellyfinItem, server_id: &str) -> MediaItem {
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
    let name = item
        .name
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
        server_id: server_id.to_string(),
    }
}

/// Read connection parameters from AppState.
fn get_connection_params(state: &AppState) -> Result<(String, String, String, String), JfgoatError> {
    let server_url = state
        .server_url
        .read()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?
        .clone()
        .ok_or_else(|| JfgoatError::Auth("No server connected".to_string()))?;
    let token = state
        .token
        .read()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?
        .clone()
        .ok_or_else(|| JfgoatError::Auth("No token".to_string()))?;
    let user_id = state
        .user_id
        .read()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?
        .clone()
        .ok_or_else(|| JfgoatError::Auth("No user ID".to_string()))?;
    let device_id: String = {
        let db = state
            .db
            .lock()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;
        db.query_row(
            "SELECT value FROM metadata WHERE key = 'device_id'",
            [],
            |row| row.get(0),
        )?
    };
    Ok((server_url, token, user_id, device_id))
}

fn get_server_id(state: &AppState) -> Result<String, JfgoatError> {
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let sid: String = db.query_row(
        "SELECT id FROM servers WHERE is_active = 1 ORDER BY connected_at DESC LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    Ok(sid)
}

// ── Local DB queries (used as fallback / for search) ────────────────────

#[tauri::command]
pub fn get_recent_movies(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items WHERE type = 'Movie' ORDER BY date_created DESC LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| row_to_media_item(row))?;

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
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items WHERE type = 'Series' ORDER BY date_created DESC LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| row_to_media_item(row))?;

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
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items
         WHERE playback_ticks > 0 AND played = 0
         ORDER BY date_updated DESC
         LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| row_to_media_item(row))?;

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
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items
         WHERE backdrop_tag IS NOT NULL AND type IN ('Movie', 'Series')
         ORDER BY date_created DESC
         LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| row_to_media_item(row))?;

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
}

/// Persist the homepage dashboard data to a JSON file for instant startup.
#[tauri::command]
pub async fn save_homepage_cache(
    app: tauri::AppHandle,
    data: HomepageCache,
) -> Result<(), JfgoatError> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let _ = fs::create_dir_all(&cache_dir);
    let cache_path = cache_dir.join("homepage_cache.json");

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
) -> Result<Option<HomepageCache>, JfgoatError> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let cache_path = cache_dir.join("homepage_cache.json");

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
        .map(|item| jf_item_to_media_item(item, &server_id))
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
        .map(|item| jf_item_to_media_item(item, &server_id))
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
    // 1. Try local DB first (fast path)
    {
        let db = state
            .db
            .lock()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;

        let sql = format!(
            "SELECT {} FROM media_items WHERE id = ?1",
            SELECT_COLUMNS
        );
        let mut stmt = db.prepare(&sql)?;
        let result = stmt.query_row(rusqlite::params![id], |row| row_to_media_item(row));

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
            Ok(jf_item) => return Ok(Some(jf_item_to_media_item(jf_item, &sid))),
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
    // 1. Try local DB first
    {
        let db = state
            .db
            .lock()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;

        let sql = format!(
            "SELECT {} FROM media_items WHERE series_id = ?1 AND type = 'Season' ORDER BY index_number ASC",
            SELECT_COLUMNS
        );
        let mut stmt = db.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params![series_id], |row| row_to_media_item(row))?;

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
                    .map(|item| jf_item_to_media_item(item, &sid))
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
    // 1. Try local DB first
    let series_id_for_fallback: Option<String>;
    {
        let db = state
            .db
            .lock()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;

        let sql = format!(
            "SELECT {} FROM media_items WHERE season_id = ?1 AND type = 'Episode' ORDER BY index_number ASC",
            SELECT_COLUMNS
        );
        let mut stmt = db.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params![season_id], |row| row_to_media_item(row))?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        if !items.is_empty() {
            return Ok(items);
        }

        // For API fallback we need the series_id; try to get it from the season record
        series_id_for_fallback = db
            .query_row(
                "SELECT series_id FROM media_items WHERE id = ?1",
                rusqlite::params![season_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .unwrap_or(None);
    }

    // 2. Fallback: fetch from Jellyfin API
    if let Some(ref sid_for_api) = series_id_for_fallback {
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
                        .map(|item| jf_item_to_media_item(item, &sid))
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
        .map(|item| jf_item_to_media_item(item, &server_id))
        .collect();

    Ok(items)
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
        .map(|item| jf_item_to_media_item(item, &server_id))
        .collect();

    Ok(items)
}
