use std::fs;
use tauri::{Manager, State};

use crate::api::media as media_api;
use crate::db::media::{
    MediaItem,
    query_local_library_items_by_parent, query_local_library_items_by_server_type,
    get_recent_movies_db, get_recent_series_db, get_continue_watching_db, get_latest_media_db,
    get_local_item_by_id, get_local_seasons, get_local_episodes, get_series_id_for_season,
    is_db_lock_contention,
};
use crate::error::JfgoatError;
use crate::state::AppState;
use super::media_types::{Person, UserLibrary, HomepageCache};

fn get_active_scope(state: &AppState) -> Result<(String, String), JfgoatError> {
    let server_id = state.get_server_id()?;
    let user_id = state
        .user_id
        .read()
        .clone()
        .ok_or_else(|| JfgoatError::Auth("No user ID".to_string()))?;

    Ok((server_id, user_id))
}

fn get_library_items_from_local_fallback(
    state: &AppState,
    parent_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<media_api::PaginatedResult<MediaItem>, JfgoatError> {
    let (server_id, user_id) = get_active_scope(state)?;
    let db = state
        .db
        .read_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let by_parent = query_local_library_items_by_parent(
        &db,
        parent_id,
        &server_id,
        &user_id,
        start_index,
        limit,
    )?;

    if by_parent.total_record_count > 0 {
        return Ok(by_parent);
    }

    query_local_library_items_by_server_type(&db, &server_id, &user_id, start_index, limit)
}

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

    get_recent_movies_db(&db, &server_id, &user_id, limit)
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

    get_recent_series_db(&db, &server_id, &user_id, limit)
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

    get_continue_watching_db(&db, &server_id, &user_id, limit)
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

    get_latest_media_db(&db, &server_id, &user_id, limit)
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
    let (jf_client, user_id, _) = state.get_jf_client()?;

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
    let (jf_client, user_id, server_id) = state.get_jf_client()?;

    let response = media_api::fetch_resume_items(&jf_client, &user_id, limit).await?;

    let items: Vec<MediaItem> = response
        .items
        .into_iter()
        .map(|item| MediaItem::from_jellyfin_item(item, &server_id, &user_id))
        .collect();

    Ok(items)
}

/// Fetch next up episodes directly from the Jellyfin server.
#[tauri::command]
pub async fn get_next_up(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let (jf_client, user_id, server_id) = state.get_jf_client()?;

    let response = media_api::fetch_next_up(&jf_client, &user_id, limit).await?;

    let items: Vec<MediaItem> = response
        .items
        .into_iter()
        .map(|item| MediaItem::from_jellyfin_item(item, &server_id, &user_id))
        .collect();

    Ok(items)
}

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

        if let Ok(Some(item)) = get_local_item_by_id(&db, &id, &scope_server_id, &scope_user_id) {
            return Ok(Some(item));
        }
    }

    // 2. Fallback: fetch from Jellyfin API
    if let Ok((jf_client, user_id, sid)) = state.get_jf_client() {
        match media_api::fetch_item_by_id(&jf_client, &user_id, &id).await {
            Ok(jf_item) => return Ok(Some(MediaItem::from_jellyfin_item(jf_item, &sid, &user_id))),
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

        if let Ok(items) = get_local_seasons(&db, &series_id, &scope_server_id, &scope_user_id) {
            if !items.is_empty() {
                return Ok(items);
            }
        }
    }

    // 2. Fallback: fetch from Jellyfin API
    if let Ok((jf_client, user_id, sid)) = state.get_jf_client() {
        match media_api::fetch_seasons(&jf_client, &user_id, &series_id).await {
            Ok(response) => {
                let items: Vec<MediaItem> = response
                    .items
                    .into_iter()
                    .map(|item| MediaItem::from_jellyfin_item(item, &sid, &user_id))
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
    let series_id_for_fallback: Option<String> = {
        let db = state
            .db
            .read_conn()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;

        match get_local_episodes(&db, &season_id, &scope_server_id, &scope_user_id) {
            Ok(items) => {
                if !items.is_empty() {
                    return Ok(items);
                }
            }
            Err(ref e) if is_db_lock_contention(e) => {
                println!(
                    "[detail] Local season episode query hit SQLite contention; falling back to API: {}",
                    e
                );
            }
            Err(e) => return Err(e.into()),
        }

        // For API fallback we need the series_id; try to get it from the season record.
        // During sync writes we tolerate transient lock contention and continue with API fallback.
        match get_series_id_for_season(&db, &season_id, &scope_server_id, &scope_user_id) {
            Ok(value) => value,
            Err(ref e) if is_db_lock_contention(e) => {
                println!(
                    "[detail] Local season->series lookup hit SQLite contention; falling back to API: {}",
                    e
                );
                None
            }
            Err(e) => return Err(e.into()),
        }
    };

    // 2. Fallback: fetch from Jellyfin API
    //    If we don't have the series_id from the local DB (season not synced),
    //    fetch the season item from the API first to discover its series_id.
    let mut series_id_resolved = series_id_for_fallback;
    if series_id_resolved.is_none() {
        if let Ok((jf_client, user_id, _)) = state.get_jf_client() {
            if let Ok(season_item) = media_api::fetch_item_by_id(&jf_client, &user_id, &season_id).await {
                series_id_resolved = season_item.series_id;
            }
        }
    }

    if let Some(ref sid_for_api) = series_id_resolved {
        if let Ok((jf_client, user_id, sid)) = state.get_jf_client() {
            match media_api::fetch_episodes(&jf_client, &user_id, sid_for_api, &season_id).await {
                Ok(response) => {
                    let items: Vec<MediaItem> = response
                        .items
                        .into_iter()
                        .map(|item| MediaItem::from_jellyfin_item(item, &sid, &user_id))
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
    let (jf_client, user_id, server_id) = state.get_jf_client()?;

    let items_raw =
        media_api::fetch_latest_items(&jf_client, &user_id, &parent_id, limit).await?;

    let items: Vec<MediaItem> = items_raw
        .into_iter()
        .map(|item| MediaItem::from_jellyfin_item(item, &server_id, &user_id))
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

    let (jf_client, user_id, server_id) = state.get_jf_client()?;

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
        .map(|item| MediaItem::from_jellyfin_item(item, &server_id, &user_id))
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
    let (jf_client, user_id, _) = state.get_jf_client()?;

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
    let (jf_client, user_id, server_id) = state.get_jf_client()?;

    let response = media_api::fetch_similar_items(&jf_client, &user_id, &id, limit).await?;

    let items: Vec<MediaItem> = response
        .items
        .into_iter()
        .map(|item| MediaItem::from_jellyfin_item(item, &server_id, &user_id))
        .collect();

    Ok(items)
}

/// Force-refresh a media item directly from the Jellyfin API and write all changes/new items
/// (including all child seasons/episodes for series) into the local SQLite database.
#[tauri::command]
pub async fn refresh_item_details(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), JfgoatError> {
    let (jf_client, user_id, server_id) = state.get_jf_client()?;

    // 1. Fetch item from remote Jellyfin API
    let jf_item = media_api::fetch_item_by_id(&jf_client, &user_id, &id).await?;
    let item_type = jf_item.item_type.clone();
    let media_item = MediaItem::from_jellyfin_item(jf_item, &server_id, &user_id);

    let mut items_to_save = vec![media_item.clone()];

    // 2. Fetch seasons/episodes recursively if needed
    if item_type == "Series" {
        let mut start = 0u32;
        let limit = 500u32;
        loop {
            let resp = media_api::fetch_series_children(&jf_client, &user_id, &id, start, limit).await?;
            let count = resp.items.len();
            if count == 0 {
                break;
            }
            for item in resp.items {
                items_to_save.push(MediaItem::from_jellyfin_item(item, &server_id, &user_id));
            }
            if count < limit as usize {
                break;
            }
            start += limit;
        }
    } else if item_type == "Season" {
        if let Some(ref series_id) = media_item.series_id {
            let resp = media_api::fetch_episodes(&jf_client, &user_id, series_id, &id).await?;
            for item in resp.items {
                items_to_save.push(MediaItem::from_jellyfin_item(item, &server_id, &user_id));
            }
        }
    }

    // 3. Save all fetched items to the DB
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        crate::db::media::insert_media_chunk(&db, &items_to_save)?;
    }

    Ok(())
}
