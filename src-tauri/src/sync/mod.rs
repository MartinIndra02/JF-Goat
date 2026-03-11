use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use tauri::{AppHandle, Emitter, Manager};
use serde::Serialize;

use crate::api::client::JellyfinClient;
use crate::api::media;
use crate::db::media::{
    insert_media_chunk, get_local_item_count,
    get_checkpoint, init_checkpoint, update_checkpoint_index, complete_checkpoint,
    CheckpointStatus, MediaItem,
};
use crate::state::{AppState, SyncStatus};

const CHUNK_SIZE: u32 = 1000;
const SERIES_CHUNK_SIZE: u32 = 500;
const SERIES_CHILDREN_LIMIT: u32 = 500;
const RATE_LIMIT_MS: u64 = 500;
const HIERARCHICAL_RATE_LIMIT_MS: u64 = 250;
const MAX_RETRIES: u32 = 4;
const MAX_CONSECUTIVE_FAILURES: u32 = 10;
const TV_CHUNK_SIZE: usize = 10;

#[derive(Debug, Clone, Serialize)]
pub struct SyncProgress {
    pub current: u32,
    pub total: u32,
    pub percentage: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncError {
    pub message: String,
    pub batch_start: u32,
    pub batch_size: u32,
    pub retries_attempted: u32,
    pub is_fatal: bool,
}

/// Convert a Jellyfin API item into our local MediaItem struct.
fn to_media_item(item: media::JellyfinItem, server_id: &str) -> MediaItem {
    let image_tag = item.image_tags.and_then(|t| t.primary);
    let backdrop_tag = item.backdrop_image_tags.and_then(|v| v.into_iter().next());
    let genres = item.genres.map(|g| g.join(", "));

    let name = item.name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("[{}]", &item.id));

    let (played, play_count, playback_ticks, is_favorite) = match item.user_data {
        Some(ud) => (
            ud.played.unwrap_or(false),
            ud.play_count.unwrap_or(0),
            ud.playback_position_ticks.unwrap_or(0),
            ud.is_favorite.unwrap_or(false),
        ),
        None => (false, 0, 0, false),
    };

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

/// Spawn the background indexing worker. Call this after successful authentication.
pub fn start_background_sync(app: AppHandle) {
    tokio::spawn(async move {
        if let Err(e) = run_sync(&app).await {
            eprintln!("Background sync failed: {}", e);
            // Ensure we never leave the status stuck at InitialSync
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut status) = state.sync_status.write() {
                    *status = SyncStatus::Ready;
                }
            }
            let _ = app.emit("sync-error", SyncError {
                message: e.to_string(),
                batch_start: 0,
                batch_size: 0,
                retries_attempted: 0,
                is_fatal: true,
            });
        }
    });
}

async fn run_sync(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Set status to INITIAL_SYNC
    {
        let mut status = state.sync_status.write().map_err(|e| e.to_string())?;
        *status = SyncStatus::InitialSync;
    }

    // Read connection parameters from AppState
    let (server_url, token, user_id, device_id) = {
        let url = state
            .server_url
            .read()
            .map_err(|e| e.to_string())?
            .clone()
            .ok_or("No server URL")?;
        let tok = state
            .token
            .read()
            .map_err(|e| e.to_string())?
            .clone()
            .ok_or("No token")?;
        let uid = state
            .user_id
            .read()
            .map_err(|e| e.to_string())?
            .clone()
            .ok_or("No user ID")?;

        let db = state.db.lock().map_err(|e| e.to_string())?;
        let did: String = db
            .query_row(
                "SELECT value FROM metadata WHERE key = 'device_id'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        (url, tok, uid, did)
    };

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    // Step 1: Fetch the absolute grand total across the entire library with a single API call
    let grand_total = media::fetch_total_item_count(&jf_client, &user_id)
        .await
        .map_err(|e| e.to_string())?;

    if grand_total == 0 {
        let mut status = state.sync_status.write().map_err(|e| e.to_string())?;
        *status = SyncStatus::Ready;
        let _ = app.emit("sync-complete", ());
        return Ok(());
    }

    println!("Grand total item count from server: {}", grand_total);

    // Step 2: Initialize global_ingested from DB for accurate resume progress
    let initial_count = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        get_local_item_count(&db).map_err(|e| e.to_string())?
    };

    println!("Resuming with {} items already in local DB", initial_count);

    // Step 3: Fetch user views (libraries)
    let views_response = media::fetch_user_views(&jf_client, &user_id)
        .await
        .map_err(|e| e.to_string())?;

    let views = views_response.items;
    if views.is_empty() {
        let mut status = state.sync_status.write().map_err(|e| e.to_string())?;
        *status = SyncStatus::Ready;
        let _ = app.emit("sync-complete", ());
        return Ok(());
    }

    // Step 4: Get per-view totals (for pagination bounds only, not for progress)
    let mut view_totals: Vec<(String, String, Option<String>, u32)> = Vec::new();

    for view in &views {
        let view_name = view.name.clone().unwrap_or_else(|| format!("[{}]", &view.id));
        let ctype = view.collection_type.clone();

        let count = if ctype.as_deref() == Some("tvshows") {
            match media::fetch_series(&jf_client, &user_id, &view.id, 0, 1, true).await {
                Ok(resp) => {
                    println!("  Library '{}' (tvshows): {} series", view_name, resp.total_record_count);
                    resp.total_record_count
                }
                Err(e) => {
                    eprintln!("  Failed to get series count for library '{}': {}", view_name, e);
                    0
                }
            }
        } else {
            match media::fetch_view_items(&jf_client, &user_id, &view.id, 0, 1).await {
                Ok(resp) => {
                    println!("  Library '{}': {} items", view_name, resp.total_record_count);
                    resp.total_record_count
                }
                Err(e) => {
                    eprintln!("  Failed to get count for library '{}': {}", view_name, e);
                    0
                }
            }
        };

        view_totals.push((view.id.clone(), view_name, ctype, count));
        tokio::time::sleep(Duration::from_millis(RATE_LIMIT_MS)).await;
    }

    println!("Starting sync: {} total items across {} libraries", grand_total, view_totals.len());

    // Get server_id for the media items
    let server_id = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let sid: String = db
            .query_row(
                "SELECT id FROM servers WHERE is_active = 1 ORDER BY connected_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        sid
    };

    // Step 5: Sync each library view with checkpointing
    let global_ingested = Arc::new(AtomicU32::new(initial_count));
    let mut global_failed_batches: u32 = 0;
    let mut global_consecutive_failures: u32 = 0;
    let mut current_rate_limit = RATE_LIMIT_MS;
    let mut abort = false;

    // Emit initial progress so the UI is correct on resume
    if initial_count > 0 {
        let percentage = (initial_count as f32 / grand_total as f32) * 100.0;
        let _ = app.emit("sync-progress", SyncProgress {
            current: initial_count,
            total: grand_total,
            percentage,
        });
    }

    for (view_id, view_name, collection_type, view_total) in &view_totals {
        if abort {
            break;
        }

        if *view_total == 0 {
            continue;
        }

        // ── Checkpoint: Check if this view was already synced ──
        let start_index = {
            let db = state.db.lock().map_err(|e| e.to_string())?;
            match get_checkpoint(&db, view_id).map_err(|e| e.to_string())? {
                CheckpointStatus::Completed => {
                    println!("Skipping library '{}' (already completed)", view_name);
                    continue;
                }
                CheckpointStatus::InProgress(last_index) => {
                    println!("Resuming library '{}' from index {}", view_name, last_index);
                    last_index
                }
                CheckpointStatus::NotFound => {
                    init_checkpoint(&db, view_id).map_err(|e| e.to_string())?;
                    println!("Starting library '{}' from index 0", view_name);
                    0
                }
            }
        };

        if collection_type.as_deref() == Some("tvshows") {
            // ── Path B: Hierarchical Sync (TV Shows) with Safe Chunking ──
            println!("Syncing TV library hierarchically: {} ({} series)", view_name, view_total);

            // B.1: Fetch ALL Series for this view (sequential — series count is small)
            let mut series_items: Vec<MediaItem> = Vec::new();
            let mut series_start: u32 = 0;

            loop {
                if abort {
                    break;
                }

                let mut batch_result = None;
                let mut last_error = String::new();

                for attempt in 0..=MAX_RETRIES {
                    if attempt > 0 {
                        let backoff_secs = 5u64 * 2u64.pow(attempt - 1);
                        eprintln!(
                            "Retry {}/{} for '{}' series batch at index {} (waiting {}s)",
                            attempt, MAX_RETRIES, view_name, series_start, backoff_secs
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    }

                    match media::fetch_series(&jf_client, &user_id, view_id, series_start, SERIES_CHUNK_SIZE, false).await {
                        Ok(response) => {
                            batch_result = Some(response);
                            break;
                        }
                        Err(e) => {
                            last_error = e.to_string();
                            eprintln!(
                                "Series batch fetch failed ('{}' index={}, attempt={}): {}",
                                view_name, series_start, attempt, last_error
                            );
                        }
                    }
                }

                match batch_result {
                    Some(response) => {
                        let chunk_count = response.items.len() as u32;
                        if chunk_count == 0 {
                            break;
                        }

                        let media: Vec<MediaItem> = response
                            .items
                            .into_iter()
                            .map(|item| to_media_item(item, &server_id))
                            .collect();

                        // Insert Series into DB
                        match state.db.lock() {
                            Ok(db) => {
                                if let Err(e) = insert_media_chunk(&db, &media) {
                                    eprintln!("DB insert failed for '{}' series batch: {}", view_name, e);
                                    global_failed_batches += 1;
                                    global_consecutive_failures += 1;
                                } else {
                                    let added = chunk_count;
                                    let current = global_ingested.fetch_add(added, Ordering::SeqCst) + added;
                                    global_consecutive_failures = 0;
                                    series_items.extend(media);

                                    let percentage = (current as f32 / grand_total as f32) * 100.0;
                                    let _ = app.emit("sync-progress", SyncProgress {
                                        current,
                                        total: grand_total,
                                        percentage,
                                    });
                                }
                            }
                            Err(e) => {
                                eprintln!("Database mutex poisoned: {}", e);
                                let _ = app.emit("sync-error", SyncError {
                                    message: format!("Database lock failed: {}", e),
                                    batch_start: series_start,
                                    batch_size: SERIES_CHUNK_SIZE,
                                    retries_attempted: 0,
                                    is_fatal: true,
                                });
                                abort = true;
                                break;
                            }
                        }

                        series_start += SERIES_CHUNK_SIZE;

                        if chunk_count < SERIES_CHUNK_SIZE {
                            break;
                        }

                        tokio::time::sleep(Duration::from_millis(current_rate_limit)).await;
                    }
                    None => {
                        global_failed_batches += 1;
                        global_consecutive_failures += 1;
                        current_rate_limit = (current_rate_limit * 2).min(5000);
                        eprintln!(
                            "Skipping '{}' series batch at index {} after {} retries: {}",
                            view_name, series_start, MAX_RETRIES, last_error
                        );
                        let _ = app.emit("sync-error", SyncError {
                            message: last_error,
                            batch_start: series_start,
                            batch_size: SERIES_CHUNK_SIZE,
                            retries_attempted: MAX_RETRIES,
                            is_fatal: false,
                        });
                        series_start += SERIES_CHUNK_SIZE;
                    }
                }

                if global_consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    eprintln!("Aborting sync after {} consecutive failures", MAX_CONSECUTIVE_FAILURES);
                    let _ = app.emit("sync-error", SyncError {
                        message: format!("Sync aborted after {} consecutive batch failures", MAX_CONSECUTIVE_FAILURES),
                        batch_start: series_start,
                        batch_size: SERIES_CHUNK_SIZE,
                        retries_attempted: MAX_RETRIES,
                        is_fatal: true,
                    });
                    abort = true;
                    break;
                }
            }

            if abort {
                break;
            }

            // B.2: Fetch Episodes per Series — Safe Chunking with join_all
            // The checkpoint's last_index tracks how many series have been fully processed.
            let mut current_series_index = start_index as usize;

            let remaining_series = if current_series_index < series_items.len() {
                &series_items[current_series_index..]
            } else {
                &[]
            };

            if !remaining_series.is_empty() {
                println!(
                    "Fetching episodes for {} remaining series in '{}' (chunks of {}, resuming from series {})",
                    remaining_series.len(), view_name, TV_CHUNK_SIZE, current_series_index
                );
            }

            for chunk in remaining_series.chunks(TV_CHUNK_SIZE) {
                // A. Create tasks for this specific chunk
                let tasks: Vec<_> = chunk.iter().map(|series| {
                    let ingested = Arc::clone(&global_ingested);
                    let app_h = app.clone();
                    let jf = JellyfinClient::new(&state.http_client, &server_url, &device_id)
                        .with_token(&token);
                    let uid = user_id.clone();
                    let sid = server_id.clone();
                    let series_id = series.id.clone();
                    let series_name = series.name.clone();
                    let db_mutex = &state.db;

                    async move {
                        let mut ep_start: u32 = 0;

                        loop {
                            let mut batch_result = None;
                            let mut last_error = String::new();

                            for attempt in 0..=MAX_RETRIES {
                                if attempt > 0 {
                                    let backoff_secs = 5u64 * 2u64.pow(attempt - 1);
                                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                                }

                                match media::fetch_series_children(
                                    &jf, &uid, &series_id, ep_start, SERIES_CHILDREN_LIMIT,
                                ).await {
                                    Ok(response) => {
                                        batch_result = Some(response);
                                        break;
                                    }
                                    Err(e) => {
                                        last_error = e.to_string();
                                        eprintln!(
                                            "Episode fetch failed for '{}' (attempt={}): {}",
                                            series_name, attempt, last_error
                                        );
                                    }
                                }
                            }

                            match batch_result {
                                Some(response) => {
                                    let chunk_count = response.items.len() as u32;
                                    if chunk_count == 0 {
                                        break;
                                    }

                                    let media_items: Vec<MediaItem> = response
                                        .items
                                        .into_iter()
                                        .map(|item| to_media_item(item, &sid))
                                        .collect();

                                    // Acquire DB lock, insert, then immediately release
                                    match db_mutex.lock() {
                                        Ok(db) => {
                                            if let Err(e) = insert_media_chunk(&db, &media_items) {
                                                eprintln!("DB insert failed for episodes of '{}': {}", series_name, e);
                                            } else {
                                                let added = chunk_count;
                                                let current = ingested.fetch_add(added, Ordering::SeqCst) + added;

                                                let percentage = (current as f32 / grand_total as f32) * 100.0;
                                                println!(
                                                    "Sync progress: {}/{} ({:.1}%) [episodes: '{}' +{}]",
                                                    current, grand_total, percentage, series_name, added
                                                );
                                                let _ = app_h.emit("sync-progress", SyncProgress {
                                                    current,
                                                    total: grand_total,
                                                    percentage,
                                                });
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Database mutex poisoned: {}", e);
                                            let _ = app_h.emit("sync-error", SyncError {
                                                message: format!("Database lock failed: {}", e),
                                                batch_start: ep_start,
                                                batch_size: SERIES_CHILDREN_LIMIT,
                                                retries_attempted: 0,
                                                is_fatal: true,
                                            });
                                            return; // Exit this series task
                                        }
                                    }

                                    if chunk_count < SERIES_CHILDREN_LIMIT {
                                        break;
                                    }

                                    ep_start += SERIES_CHILDREN_LIMIT;
                                    tokio::time::sleep(Duration::from_millis(HIERARCHICAL_RATE_LIMIT_MS)).await;
                                }
                                None => {
                                    eprintln!(
                                        "Skipping episodes for '{}' at index {} after {} retries",
                                        series_name, ep_start, MAX_RETRIES
                                    );
                                    let _ = app_h.emit("sync-error", SyncError {
                                        message: last_error,
                                        batch_start: ep_start,
                                        batch_size: SERIES_CHILDREN_LIMIT,
                                        retries_attempted: MAX_RETRIES,
                                        is_fatal: false,
                                    });
                                    break; // Move on to next series
                                }
                            }
                        }

                        // Rate limit so concurrent workers don't overwhelm the server
                        tokio::time::sleep(Duration::from_millis(HIERARCHICAL_RATE_LIMIT_MS)).await;
                    }
                }).collect();

                // B. WAIT for all series in this chunk to completely finish
                join_all(tasks).await;

                // C. Safely advance checkpoint ONLY after the whole chunk is in the DB
                current_series_index += chunk.len();

                // D. Update SQLite checkpoint
                match state.db.lock() {
                    Ok(db) => {
                        if let Err(e) = update_checkpoint_index(&db, view_id, current_series_index as u32) {
                            eprintln!("Failed to update checkpoint for '{}': {}", view_name, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Database mutex poisoned during checkpoint update: {}", e);
                        abort = true;
                        break;
                    }
                }
            }

            // Mark TV library as COMPLETED
            if !abort {
                match state.db.lock() {
                    Ok(db) => {
                        if let Err(e) = complete_checkpoint(&db, view_id) {
                            eprintln!("Failed to complete checkpoint for '{}': {}", view_name, e);
                        } else {
                            println!("Library '{}' sync completed", view_name);
                        }
                    }
                    Err(e) => {
                        eprintln!("Database mutex poisoned during checkpoint complete: {}", e);
                    }
                }
            }
        } else {
            // ── Path A: Flat Sync (Movies, Playlists, BoxSets, etc.) ──
            println!("Syncing library: {} ({} items, starting at {})", view_name, view_total, start_index);

            let mut current_index = start_index;

            while current_index < *view_total {
                // Fetch batch with retry logic
                let mut batch_result = None;
                let mut last_error = String::new();

                for attempt in 0..=MAX_RETRIES {
                    if attempt > 0 {
                        let backoff_secs = 5u64 * 2u64.pow(attempt - 1); // 5s, 10s, 20s, 40s
                        eprintln!(
                            "Retry {}/{} for '{}' batch at index {} (waiting {}s)",
                            attempt, MAX_RETRIES, view_name, current_index, backoff_secs
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    }

                    match media::fetch_view_items_no_count(&jf_client, &user_id, view_id, current_index, CHUNK_SIZE).await {
                        Ok(response) => {
                            batch_result = Some(response);
                            break;
                        }
                        Err(e) => {
                            last_error = e.to_string();
                            eprintln!(
                                "Batch fetch failed ('{}' index={}, attempt={}): {}",
                                view_name, current_index, attempt, last_error
                            );
                        }
                    }
                }

                // Process the batch result
                match batch_result {
                    Some(response) => {
                        let chunk_count = response.items.len() as u32;
                        if chunk_count == 0 {
                            break;
                        }

                        let media_items: Vec<MediaItem> = response
                            .items
                            .into_iter()
                            .map(|item| to_media_item(item, &server_id))
                            .collect();

                        match state.db.lock() {
                            Ok(db) => {
                                if let Err(e) = insert_media_chunk(&db, &media_items) {
                                    eprintln!(
                                        "DB insert failed for '{}' batch at index {}: {}",
                                        view_name, current_index, e
                                    );
                                    global_failed_batches += 1;
                                    global_consecutive_failures += 1;
                                    let _ = app.emit("sync-error", SyncError {
                                        message: format!("Database insert failed: {}", e),
                                        batch_start: current_index,
                                        batch_size: CHUNK_SIZE,
                                        retries_attempted: 0,
                                        is_fatal: false,
                                    });
                                } else {
                                    let added = chunk_count;
                                    let current = global_ingested.fetch_add(added, Ordering::SeqCst) + added;
                                    global_consecutive_failures = 0;
                                    current_rate_limit = RATE_LIMIT_MS;

                                    // Update checkpoint after successful DB insert
                                    let next_index = current_index + CHUNK_SIZE;
                                    if let Err(e) = update_checkpoint_index(&db, view_id, next_index) {
                                        eprintln!("Failed to update checkpoint for '{}': {}", view_name, e);
                                    }

                                    let percentage = (current as f32 / grand_total as f32) * 100.0;
                                    println!(
                                        "Sync progress: {}/{} ({:.1}%) [{}] [failed batches: {}]",
                                        current, grand_total, percentage, view_name, global_failed_batches
                                    );
                                    let _ = app.emit("sync-progress", SyncProgress {
                                        current,
                                        total: grand_total,
                                        percentage,
                                    });
                                }
                            }
                            Err(e) => {
                                eprintln!("Database mutex poisoned: {}", e);
                                let _ = app.emit("sync-error", SyncError {
                                    message: format!("Database lock failed: {}", e),
                                    batch_start: current_index,
                                    batch_size: CHUNK_SIZE,
                                    retries_attempted: 0,
                                    is_fatal: true,
                                });
                                abort = true;
                                break;
                            }
                        }
                    }
                    None => {
                        // All retries exhausted -- skip this batch
                        global_failed_batches += 1;
                        global_consecutive_failures += 1;
                        current_rate_limit = (current_rate_limit * 2).min(5000);
                        eprintln!(
                            "Skipping '{}' batch at index {} after {} retries: {}",
                            view_name, current_index, MAX_RETRIES, last_error
                        );
                        let _ = app.emit("sync-error", SyncError {
                            message: last_error,
                            batch_start: current_index,
                            batch_size: CHUNK_SIZE,
                            retries_attempted: MAX_RETRIES,
                            is_fatal: false,
                        });
                    }
                }

                // Circuit breaker
                if global_consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    eprintln!(
                        "Aborting sync after {} consecutive failures in '{}' at index {}",
                        MAX_CONSECUTIVE_FAILURES, view_name, current_index
                    );
                    let _ = app.emit("sync-error", SyncError {
                        message: format!(
                            "Sync aborted after {} consecutive batch failures",
                            MAX_CONSECUTIVE_FAILURES
                        ),
                        batch_start: current_index,
                        batch_size: CHUNK_SIZE,
                        retries_attempted: MAX_RETRIES,
                        is_fatal: true,
                    });
                    abort = true;
                    break;
                }

                // Always advance
                current_index += CHUNK_SIZE;

                // Rate limiting
                if current_index < *view_total {
                    tokio::time::sleep(Duration::from_millis(current_rate_limit)).await;
                }
            }

            // Mark flat library as COMPLETED
            if !abort {
                match state.db.lock() {
                    Ok(db) => {
                        if let Err(e) = complete_checkpoint(&db, view_id) {
                            eprintln!("Failed to complete checkpoint for '{}': {}", view_name, e);
                        } else {
                            println!("Library '{}' sync completed", view_name);
                        }
                    }
                    Err(e) => {
                        eprintln!("Database mutex poisoned during checkpoint complete: {}", e);
                    }
                }
            }
        }

        // Brief pause between libraries
        if !abort {
            tokio::time::sleep(Duration::from_millis(RATE_LIMIT_MS)).await;
        }
    }

    // Step 6: Mark as READY
    {
        let mut status = state.sync_status.write().map_err(|e| e.to_string())?;
        *status = SyncStatus::Ready;
    }

    let final_ingested = global_ingested.load(Ordering::SeqCst);

    if global_failed_batches > 0 {
        println!(
            "Sync completed with errors: {} items indexed, {} batches failed",
            final_ingested, global_failed_batches
        );
        let _ = app.emit("sync-complete-with-errors", SyncError {
            message: format!(
                "Indexed {} items. {} batches could not be synced.",
                final_ingested,
                global_failed_batches
            ),
            batch_start: 0,
            batch_size: 0,
            retries_attempted: 0,
            is_fatal: false,
        });
    } else {
        println!("Sync complete: {} items indexed across {} libraries", final_ingested, view_totals.len());
    }

    let _ = app.emit("sync-complete", ());

    Ok(())
}
