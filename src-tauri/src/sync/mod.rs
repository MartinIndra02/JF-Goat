use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use serde::Serialize;

use crate::api::client::JellyfinClient;
use crate::api::media;
use crate::db::media::{insert_media_chunk, MediaItem};
use crate::state::{AppState, SyncStatus};

const CHUNK_SIZE: u32 = 1000;
const SERIES_CHUNK_SIZE: u32 = 500;
const SERIES_CHILDREN_LIMIT: u32 = 500;
const RATE_LIMIT_MS: u64 = 500;
const HIERARCHICAL_RATE_LIMIT_MS: u64 = 200;
const MAX_RETRIES: u32 = 4;
const MAX_CONSECUTIVE_FAILURES: u32 = 10;

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

    // Step 1: Fetch user views (libraries)
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

    // Step 2: Pre-calculate grand total across all views
    // (id, name, collection_type, total)
    let mut view_totals: Vec<(String, String, Option<String>, u32)> = Vec::new();
    let mut grand_total: u32 = 0;

    for view in &views {
        let view_name = view.name.clone().unwrap_or_else(|| format!("[{}]", &view.id));
        let ctype = view.collection_type.clone();

        let count = if ctype.as_deref() == Some("tvshows") {
            // For TV Shows, count only the Series (we fetch episodes per-series later)
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

        // For tvshows, we don't know exact episode count upfront — grand_total is an estimate
        // that will be updated as we discover episodes
        grand_total += count;
        view_totals.push((view.id.clone(), view_name, ctype, count));
        tokio::time::sleep(Duration::from_millis(RATE_LIMIT_MS)).await;
    }

    if grand_total == 0 {
        let mut status = state.sync_status.write().map_err(|e| e.to_string())?;
        *status = SyncStatus::Ready;
        let _ = app.emit("sync-complete", ());
        return Ok(());
    }

    println!("Starting initial sync: {} total items across {} libraries", grand_total, view_totals.len());

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

    // Step 3: Sync each library view
    let mut global_ingested: u32 = 0;
    let mut global_failed_batches: u32 = 0;
    let mut global_consecutive_failures: u32 = 0;
    let mut current_rate_limit = RATE_LIMIT_MS;
    let mut abort = false;

    for (view_id, view_name, collection_type, view_total) in &view_totals {
        if abort {
            break;
        }

        if *view_total == 0 {
            continue;
        }

        if collection_type.as_deref() == Some("tvshows") {
            // ── Path B: Hierarchical Sync (TV Shows) ──
            println!("Syncing TV library hierarchically: {} ({} series)", view_name, view_total);

            // B.1: Fetch all Series for this view
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
                                    global_ingested += chunk_count;
                                    global_consecutive_failures = 0;
                                    series_items.extend(media);
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

                        // Emit progress after series batch
                        let percentage = if grand_total > 0 {
                            (global_ingested as f32 / grand_total as f32) * 100.0
                        } else {
                            0.0
                        };
                        let _ = app.emit("sync-progress", SyncProgress {
                            current: global_ingested,
                            total: grand_total,
                            percentage,
                        });

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

            // B.2: Fetch Episodes per Series
            println!("Fetching episodes for {} series in '{}'", series_items.len(), view_name);

            for (idx, series) in series_items.iter().enumerate() {
                if abort {
                    break;
                }

                let series_name = &series.name;
                let mut ep_start: u32 = 0;

                loop {
                    if abort {
                        break;
                    }

                    let mut batch_result = None;
                    let mut last_error = String::new();

                    for attempt in 0..=MAX_RETRIES {
                        if attempt > 0 {
                            let backoff_secs = 5u64 * 2u64.pow(attempt - 1);
                            tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                        }

                        match media::fetch_series_children(
                            &jf_client, &user_id, &series.id, ep_start, SERIES_CHILDREN_LIMIT,
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

                            let media: Vec<MediaItem> = response
                                .items
                                .into_iter()
                                .map(|item| to_media_item(item, &server_id))
                                .collect();

                            match state.db.lock() {
                                Ok(db) => {
                                    if let Err(e) = insert_media_chunk(&db, &media) {
                                        eprintln!("DB insert failed for episodes of '{}': {}", series_name, e);
                                        global_failed_batches += 1;
                                        global_consecutive_failures += 1;
                                    } else {
                                        global_ingested += chunk_count;
                                        // Update grand_total as we discover episodes
                                        grand_total += chunk_count;
                                        global_consecutive_failures = 0;
                                        current_rate_limit = RATE_LIMIT_MS;
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Database mutex poisoned: {}", e);
                                    let _ = app.emit("sync-error", SyncError {
                                        message: format!("Database lock failed: {}", e),
                                        batch_start: ep_start,
                                        batch_size: SERIES_CHILDREN_LIMIT,
                                        retries_attempted: 0,
                                        is_fatal: true,
                                    });
                                    abort = true;
                                    break;
                                }
                            }

                            // Emit progress
                            let percentage = if grand_total > 0 {
                                (global_ingested as f32 / grand_total as f32) * 100.0
                            } else {
                                0.0
                            };
                            println!(
                                "Sync progress: {}/{} ({:.1}%) [{}] series {}/{}",
                                global_ingested, grand_total, percentage, view_name, idx + 1, series_items.len()
                            );
                            let _ = app.emit("sync-progress", SyncProgress {
                                current: global_ingested,
                                total: grand_total,
                                percentage,
                            });

                            if chunk_count < SERIES_CHILDREN_LIMIT {
                                break;
                            }

                            ep_start += SERIES_CHILDREN_LIMIT;
                            tokio::time::sleep(Duration::from_millis(current_rate_limit)).await;
                        }
                        None => {
                            global_failed_batches += 1;
                            global_consecutive_failures += 1;
                            current_rate_limit = (current_rate_limit * 2).min(5000);
                            eprintln!(
                                "Skipping episodes for '{}' at index {} after {} retries",
                                series_name, ep_start, MAX_RETRIES
                            );
                            let _ = app.emit("sync-error", SyncError {
                                message: last_error,
                                batch_start: ep_start,
                                batch_size: SERIES_CHILDREN_LIMIT,
                                retries_attempted: MAX_RETRIES,
                                is_fatal: false,
                            });
                            break; // Move on to next series
                        }
                    }

                    if global_consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        eprintln!("Aborting sync after {} consecutive failures", MAX_CONSECUTIVE_FAILURES);
                        let _ = app.emit("sync-error", SyncError {
                            message: format!("Sync aborted after {} consecutive batch failures", MAX_CONSECUTIVE_FAILURES),
                            batch_start: ep_start,
                            batch_size: SERIES_CHILDREN_LIMIT,
                            retries_attempted: MAX_RETRIES,
                            is_fatal: true,
                        });
                        abort = true;
                        break;
                    }
                }

                // Rate limit between series
                if !abort {
                    tokio::time::sleep(Duration::from_millis(HIERARCHICAL_RATE_LIMIT_MS)).await;
                }
            }
        } else {
            // ── Path A: Flat Sync (Movies, Playlists, BoxSets, etc.) ──
            println!("Syncing library: {} ({} items)", view_name, view_total);

            let mut start_index: u32 = 0;

            while start_index < *view_total {
                // Fetch batch with retry logic
                let mut batch_result = None;
                let mut last_error = String::new();

                for attempt in 0..=MAX_RETRIES {
                    if attempt > 0 {
                        let backoff_secs = 5u64 * 2u64.pow(attempt - 1); // 5s, 10s, 20s, 40s
                        eprintln!(
                            "Retry {}/{} for '{}' batch at index {} (waiting {}s)",
                            attempt, MAX_RETRIES, view_name, start_index, backoff_secs
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    }

                    match media::fetch_view_items_no_count(&jf_client, &user_id, view_id, start_index, CHUNK_SIZE).await {
                        Ok(response) => {
                            batch_result = Some(response);
                            break;
                        }
                        Err(e) => {
                            last_error = e.to_string();
                            eprintln!(
                                "Batch fetch failed ('{}' index={}, attempt={}): {}",
                                view_name, start_index, attempt, last_error
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
                                        view_name, start_index, e
                                    );
                                    global_failed_batches += 1;
                                    global_consecutive_failures += 1;
                                    let _ = app.emit("sync-error", SyncError {
                                        message: format!("Database insert failed: {}", e),
                                        batch_start: start_index,
                                        batch_size: CHUNK_SIZE,
                                        retries_attempted: 0,
                                        is_fatal: false,
                                    });
                                } else {
                                    global_ingested += chunk_count;
                                    global_consecutive_failures = 0;
                                    current_rate_limit = RATE_LIMIT_MS;
                                }
                            }
                            Err(e) => {
                                eprintln!("Database mutex poisoned: {}", e);
                                let _ = app.emit("sync-error", SyncError {
                                    message: format!("Database lock failed: {}", e),
                                    batch_start: start_index,
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
                            view_name, start_index, MAX_RETRIES, last_error
                        );
                        let _ = app.emit("sync-error", SyncError {
                            message: last_error,
                            batch_start: start_index,
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
                        MAX_CONSECUTIVE_FAILURES, view_name, start_index
                    );
                    let _ = app.emit("sync-error", SyncError {
                        message: format!(
                            "Sync aborted after {} consecutive batch failures",
                            MAX_CONSECUTIVE_FAILURES
                        ),
                        batch_start: start_index,
                        batch_size: CHUNK_SIZE,
                        retries_attempted: MAX_RETRIES,
                        is_fatal: true,
                    });
                    abort = true;
                    break;
                }

                // Always advance
                start_index += CHUNK_SIZE;

                // Emit global progress
                let percentage = (global_ingested as f32 / grand_total as f32) * 100.0;
                let progress = SyncProgress {
                    current: global_ingested,
                    total: grand_total,
                    percentage,
                };

                println!(
                    "Sync progress: {}/{} ({:.1}%) [{}] [failed batches: {}]",
                    global_ingested, grand_total, percentage, view_name, global_failed_batches
                );

                let _ = app.emit("sync-progress", progress);

                // Rate limiting
                if start_index < *view_total {
                    tokio::time::sleep(Duration::from_millis(current_rate_limit)).await;
                }
            }
        }

        // Brief pause between libraries
        if !abort {
            tokio::time::sleep(Duration::from_millis(RATE_LIMIT_MS)).await;
        }
    }

    // Step 4: Mark as READY
    {
        let mut status = state.sync_status.write().map_err(|e| e.to_string())?;
        *status = SyncStatus::Ready;
    }

    if global_failed_batches > 0 {
        println!(
            "Sync completed with errors: {} items indexed, {} batches failed",
            global_ingested, global_failed_batches
        );
        let _ = app.emit("sync-complete-with-errors", SyncError {
            message: format!(
                "Indexed {} items. {} batches ({} items) could not be synced.",
                global_ingested,
                global_failed_batches,
                global_failed_batches * CHUNK_SIZE
            ),
            batch_start: 0,
            batch_size: 0,
            retries_attempted: 0,
            is_fatal: false,
        });
    } else {
        println!("Initial sync complete: {} items indexed across {} libraries", global_ingested, view_totals.len());
    }

    let _ = app.emit("sync-complete", ());

    Ok(())
}
