use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use tauri::{AppHandle, Emitter, Manager};
use serde::Serialize;

use crate::api::client::JellyfinClient;
use crate::api::media;
use crate::commands::media_commands::apply_user_data_refresh_batch;
use crate::db::media::{
    insert_media_chunk, get_local_item_count_scoped,
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
const INCREMENTAL_REFRESH_INTERVAL_SECS: u64 = 240;
const INCREMENTAL_REFRESH_BATCH_SIZE: u32 = 1000;
const INCREMENTAL_REFRESH_MAX_PAGES: u32 = 200;

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
fn to_media_item(item: media::JellyfinItem, server_id: &str, user_id: &str) -> MediaItem {
    MediaItem::from_jellyfin_item(item, server_id, user_id)
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

async fn run_sync(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();

    // Read connection parameters and server ID from AppState using helpers
    let (server_url, token, user_id, device_id) = state.get_connection_params().map_err(|e| e.to_string())?;
    let server_id = state.get_server_id().map_err(|e| e.to_string())?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    // Step 1: Fetch the absolute grand total across the entire library with a single API call
    let grand_total = media::fetch_total_item_count(&jf_client, &user_id)
        .await
        .map_err(|e| e.to_string())?;

    if grand_total == 0 {
        *state.sync_status.write() = SyncStatus::Ready;
        let _ = app.emit("sync-complete", ());
        return Ok(());
    }

    println!("Grand total item count from server: {}", grand_total);

    // Step 2: Initialize global_ingested from DB for accurate resume progress
    let initial_count = {
        let db = state.db.read_conn().map_err(|e| e.to_string())?;
        get_local_item_count_scoped(&db, Some(&server_id), Some(&user_id)).map_err(|e| e.to_string())?
    };

    println!("Resuming with {} items already in local DB", initial_count);

    // Set status to INITIAL_SYNC only if this is a fresh sync or sync is not fully complete yet.
    // If we have already successfully completed the initial sync (all checkpoints Completed),
    // we can run a fast incremental delta sync in the background without taking the database offline.
    let all_completed = {
        let db = state.db.read_conn().map_err(|e| e.to_string())?;
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
    };

    if all_completed {
        println!("All checkpoints completed for user. Performing incremental delta sync.");
        let mut start_index = 0u32;
        let limit = 300u32;
        let mut page = 0;
        let max_pages = 10; // Fetch up to 3000 items if needed

        loop {
            match media::fetch_recent_items(&jf_client, &user_id, start_index, limit).await {
                Ok(response) => {
                    let items_count = response.items.len();
                    if items_count == 0 {
                        break;
                    }

                    // Check which items from this batch are already in the DB and unmodified
                    let db = state.db.read_conn().map_err(|e| e.to_string())?;
                    let mut existing_count = 0;
                    
                    {
                        let mut stmt = db.prepare_cached(
                            "SELECT date_updated, date_created FROM media_items WHERE id = ?1 AND server_id = ?2 AND user_id = ?3"
                        ).map_err(|e| e.to_string())?;

                        for item in &response.items {
                            let db_dates: Result<(Option<String>, Option<String>), _> = stmt.query_row(
                                rusqlite::params![item.id, server_id, user_id],
                                |row| Ok((row.get(0)?, row.get(1)?))
                            );

                            if let Ok((db_updated, db_created)) = db_dates {
                                let item_created = item.date_created.as_ref();
                                let calculated_updated = item.date_last_media_added.as_ref().or(item.premiere_date.as_ref());
                                if db_created.as_ref() == item_created && db_updated.as_ref() == calculated_updated {
                                    existing_count += 1;
                                }
                            }
                        }
                    }

                    // Convert and insert/replace the items
                    let media_items: Vec<MediaItem> = response
                        .items
                        .into_iter()
                        .map(|item| to_media_item(item, &server_id, &user_id))
                        .collect();

                    {
                        let db = state.db.write_conn().map_err(|e| e.to_string())?;
                        insert_media_chunk(&db, &media_items).map_err(|e| e.to_string())?;
                    }
                    println!(
                        "Delta sync page {}: processed {} items ({} new/updated, {} unchanged)",
                        page + 1,
                        items_count,
                        items_count - existing_count,
                        existing_count
                    );

                    // If all items in this batch already exist in the DB and are unchanged, we are fully caught up!
                    if existing_count == items_count {
                        println!("Delta sync fully caught up.");
                        break;
                    }

                    start_index += limit;
                    page += 1;
                    if page >= max_pages {
                        println!("Delta sync reached max page limit (3000 items).");
                        break;
                    }

                    // Tiny pause to be friendly to the server
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                Err(e) => {
                    eprintln!("Delta sync fetch failed: {}", e);
                    break;
                }
            }
        }

        *state.sync_status.write() = SyncStatus::Ready;
        // Do not emit sync-complete for background delta syncs to avoid repeatedly flashing the success banner.
        ensure_incremental_refresh_worker(app);
        return Ok(());
    }

    // Step 3: Fetch user views (libraries)
    let views_response = media::fetch_user_views(&jf_client, &user_id)
        .await
        .map_err(|e| e.to_string())?;

    let views = views_response.items;
    if views.is_empty() {
        *state.sync_status.write() = SyncStatus::Ready;
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
            let db = state.db.read_conn().map_err(|e| e.to_string())?;
            get_checkpoint(&db, view_id, &server_id, &user_id).map_err(|e| e.to_string())?
        };

        let start_index = match start_index {
            CheckpointStatus::Completed => {
                println!("Skipping library '{}' (already completed)", view_name);
                continue;
            }
            CheckpointStatus::InProgress(last_index) => {
                println!("Resuming library '{}' from index {}", view_name, last_index);
                last_index
            }
            CheckpointStatus::NotFound => {
                let db = state.db.write_conn().map_err(|e| e.to_string())?;
                init_checkpoint(&db, view_id, &server_id, &user_id).map_err(|e| e.to_string())?;
                println!("Starting library '{}' from index 0", view_name);
                0
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
                            .map(|item| to_media_item(item, &server_id, &user_id))
                            .collect();

                        // Insert Series into DB
                        match state.db.write_conn() {
                            Ok(db) => {
                                if let Err(e) = insert_media_chunk(&db, &media) {
                                    eprintln!("DB insert failed for '{}' series batch: {}", view_name, e);
                                    global_failed_batches += 1;
                                    global_consecutive_failures += 1;
                                } else {
                                    global_consecutive_failures = 0;
                                    series_items.extend(media);

                                    // Only count series toward progress on fresh sync.
                                    // On resume (start_index > 0), these are already in
                                    // initial_count from the DB so counting them again
                                    // would push progress above 100%.
                                    if start_index == 0 {
                                        let added = chunk_count;
                                        let current = global_ingested.fetch_add(added, Ordering::SeqCst) + added;

                                        let percentage = (current as f32 / grand_total as f32) * 100.0;
                                        let _ = app.emit("sync-progress", SyncProgress {
                                            current,
                                            total: grand_total,
                                            percentage,
                                        });
                                    }
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
                // A. Fetch series episode trees concurrently, but defer DB writes until all network calls finish.
                let tasks: Vec<_> = chunk.iter().map(|series| {
                    let jf = JellyfinClient::new(&state.http_client, &server_url, &device_id)
                        .with_token(&token);
                    let uid = user_id.clone();
                    let sid = server_id.clone();
                    let series_id = series.id.clone();
                    let series_name = series.name.clone();

                    async move {
                        let mut ep_start: u32 = 0;
                        let mut series_buffer: Vec<MediaItem> = Vec::new();

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
                                        .map(|item| to_media_item(item, &sid, &uid))
                                        .collect();
                                    series_buffer.extend(media_items);

                                    if chunk_count < SERIES_CHILDREN_LIMIT {
                                        break;
                                    }

                                    ep_start += SERIES_CHILDREN_LIMIT;
                                    tokio::time::sleep(Duration::from_millis(HIERARCHICAL_RATE_LIMIT_MS)).await;
                                }
                                None => {
                                    return Err(SyncError {
                                        message: format!(
                                            "Skipping episodes for '{}' at index {} after {} retries: {}",
                                            series_name, ep_start, MAX_RETRIES, last_error
                                        ),
                                        batch_start: ep_start,
                                        batch_size: SERIES_CHILDREN_LIMIT,
                                        retries_attempted: MAX_RETRIES,
                                        is_fatal: false,
                                    });
                                }
                            }
                        }

                        // Rate limit so concurrent workers don't overwhelm the server
                        tokio::time::sleep(Duration::from_millis(HIERARCHICAL_RATE_LIMIT_MS)).await;
                        Ok::<Vec<MediaItem>, SyncError>(series_buffer)
                    }
                }).collect();

                // B. Wait for all series in this chunk to finish network fetches.
                let results = join_all(tasks).await;

                let mut chunk_media: Vec<MediaItem> = Vec::new();
                for result in results {
                    match result {
                        Ok(series_items) => {
                            if !series_items.is_empty() {
                                chunk_media.extend(series_items);
                            }
                        }
                        Err(err_payload) => {
                            global_failed_batches += 1;
                            global_consecutive_failures += 1;
                            let _ = app.emit("sync-error", err_payload);
                        }
                    }
                }

                if !chunk_media.is_empty() {
                    match state.db.write_conn() {
                        Ok(db) => {
                            if let Err(e) = insert_media_chunk(&db, &chunk_media) {
                                eprintln!("DB insert failed for '{}' hierarchical chunk: {}", view_name, e);
                                global_failed_batches += 1;
                                global_consecutive_failures += 1;
                            } else {
                                let added = chunk_media.len() as u32;
                                let current = global_ingested.fetch_add(added, Ordering::SeqCst) + added;
                                global_consecutive_failures = 0;

                                let percentage = (current as f32 / grand_total as f32) * 100.0;
                                println!(
                                    "Sync progress: {}/{} ({:.1}%) [{} episodes +{}]",
                                    current, grand_total, percentage, view_name, added
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
                                batch_start: current_series_index as u32,
                                batch_size: chunk.len() as u32,
                                retries_attempted: 0,
                                is_fatal: true,
                            });
                            abort = true;
                            break;
                        }
                    }
                }

                if global_consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    eprintln!(
                        "Aborting sync after {} consecutive failures in '{}'",
                        MAX_CONSECUTIVE_FAILURES, view_name
                    );
                    let _ = app.emit("sync-error", SyncError {
                        message: format!(
                            "Sync aborted after {} consecutive batch failures",
                            MAX_CONSECUTIVE_FAILURES
                        ),
                        batch_start: current_series_index as u32,
                        batch_size: chunk.len() as u32,
                        retries_attempted: MAX_RETRIES,
                        is_fatal: true,
                    });
                    abort = true;
                    break;
                }

                // C. Safely advance checkpoint ONLY after the whole chunk is in the DB
                current_series_index += chunk.len();

                // D. Update SQLite checkpoint
                match state.db.write_conn() {
                    Ok(db) => {
                        if let Err(e) = update_checkpoint_index(
                            &db,
                            view_id,
                            &server_id,
                            &user_id,
                            current_series_index as u32,
                        ) {
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
                match state.db.write_conn() {
                    Ok(db) => {
                        if let Err(e) = complete_checkpoint(&db, view_id, &server_id, &user_id) {
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
                            .map(|item| to_media_item(item, &server_id, &user_id))
                            .collect();

                        match state.db.write_conn() {
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
                                    if let Err(e) = update_checkpoint_index(
                                        &db,
                                        view_id,
                                        &server_id,
                                        &user_id,
                                        next_index,
                                    ) {
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
                match state.db.write_conn() {
                    Ok(db) => {
                        if let Err(e) = complete_checkpoint(&db, view_id, &server_id, &user_id) {
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
        *state.sync_status.write() = SyncStatus::Ready;
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
        let _ = app.emit("sync-complete", ());
    }

    ensure_incremental_refresh_worker(app);

    Ok(())
}

fn ensure_incremental_refresh_worker(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    if state
        .user_data_refresh_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let app_handle = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(INCREMENTAL_REFRESH_INTERVAL_SECS)).await;

            let Some(state) = app_handle.try_state::<AppState>() else {
                break;
            };

            let is_ready = *state.sync_status.read() == SyncStatus::Ready;
            if !is_ready {
                continue;
            }

            match refresh_user_data_once(&state).await {
                Ok(updated) => {
                    if updated > 0 {
                        println!("Incremental user-data refresh updated {} rows", updated);
                    }
                }
                Err(err) => {
                    eprintln!("Incremental user-data refresh failed: {}", err);
                }
            }
        }

        if let Some(state) = app_handle.try_state::<AppState>() {
            state.user_data_refresh_running.store(false, Ordering::SeqCst);
        }
    });
}

async fn refresh_user_data_once(state: &AppState) -> Result<u32, String> {
    let (server_url, token, user_id, device_id) = state.get_connection_params().map_err(|e| e.to_string())?;
    let server_id = state.get_server_id().map_err(|e| e.to_string())?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let mut start_index = 0u32;
    let mut pages = 0u32;
    let mut total_updated = 0u32;
    let mut expected_total: Option<u32> = None;

    loop {
        let enable_total_count = start_index == 0;
        let response = media::fetch_user_items_userdata(
            &jf_client,
            &user_id,
            start_index,
            INCREMENTAL_REFRESH_BATCH_SIZE,
            enable_total_count,
        )
        .await
        .map_err(|e| e.to_string())?;

        if enable_total_count {
            expected_total = Some(response.total_record_count);
        }

        if response.items.is_empty() {
            break;
        }

        let updated = apply_user_data_refresh_batch(state, &server_id, &user_id, &response.items)
            .map_err(|e| e.to_string())?;
        total_updated += updated;

        pages += 1;
        if pages >= INCREMENTAL_REFRESH_MAX_PAGES {
            println!(
                "Incremental user-data refresh reached page cap ({})",
                INCREMENTAL_REFRESH_MAX_PAGES
            );
            break;
        }

        start_index = start_index.saturating_add(INCREMENTAL_REFRESH_BATCH_SIZE);
        if let Some(total) = expected_total {
            if start_index >= total {
                break;
            }
        }
    }

    Ok(total_updated)
}
