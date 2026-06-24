use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::RwLock;
use tauri::{Emitter, Manager, State};
use futures::StreamExt;
use tokio::io::AsyncWriteExt;

use crate::error::JfgoatError;
use crate::state::{AppState, DbPool};

#[derive(Clone)]
pub struct DownloadContext {
    pub db: DbPool,
    pub http_client: reqwest::Client,
    pub server_url: Arc<RwLock<Option<String>>>,
    pub user_id: Arc<RwLock<Option<String>>>,
    pub token: Arc<RwLock<Option<String>>>,
    pub download_trigger: tokio::sync::mpsc::UnboundedSender<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineDownload {
    pub id: String,
    pub server_id: String,
    pub user_id: String,
    pub name: String,
    pub item_type: String,
    pub local_path: Option<String>,
    pub status: String, // 'Pending', 'Downloading', 'Completed', 'Paused', 'Failed', 'Cancelled'
    pub progress: f64,
    pub downloaded_bytes: i64,
    pub total_bytes: i64,
    pub speed_bytes_per_sec: f64,
    pub error_message: Option<String>,
    pub added_at: String,
}

pub async fn start_download_manager_loop(
    app_handle: tauri::AppHandle,
    ctx: DownloadContext,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<()>,
) {
    println!("[download] Starting download manager loop");
    // On startup, reset any 'Downloading' states to 'Pending' so they can be retried.
    if let Ok(db) = ctx.db.write_conn() {
        let _ = db.execute(
            "UPDATE offline_downloads SET status = 'Pending' WHERE status = 'Downloading'",
            [],
        );
    }

    loop {
        let next_item = match get_next_pending_download(&ctx) {
            Ok(Some(item)) => Some(item),
            Ok(None) => None,
            Err(e) => {
                eprintln!("[download] Error querying pending downloads: {}", e);
                None
            }
        };

        if let Some(item) = next_item {
            println!("[download] Starting download of: {}", item.name);
            if let Err(e) = download_media_item(app_handle.clone(), &ctx, &item).await {
                eprintln!("[download] Failed downloading {}: {:?}", item.id, e);
            }
        } else {
            tokio::select! {
                _ = rx.recv() => {
                    println!("[download] Wakeup trigger received");
                },
                _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {}
            }
        }
    }
}

fn get_next_pending_download(ctx: &DownloadContext) -> Result<Option<OfflineDownload>, JfgoatError> {
    let db = ctx.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let mut stmt = db.prepare(
        "SELECT id, server_id, user_id, name, type, local_path, status, progress,
                downloaded_bytes, total_bytes, speed_bytes_per_sec, error_message, added_at
         FROM offline_downloads
         WHERE status = 'Pending'
         ORDER BY added_at ASC
         LIMIT 1"
    )?;
    
    let mut rows = stmt.query_map([], |row| {
        Ok(OfflineDownload {
            id: row.get(0)?,
            server_id: row.get(1)?,
            user_id: row.get(2)?,
            name: row.get(3)?,
            item_type: row.get(4)?,
            local_path: row.get(5)?,
            status: row.get(6)?,
            progress: row.get(7)?,
            downloaded_bytes: row.get(8)?,
            total_bytes: row.get(9)?,
            speed_bytes_per_sec: row.get(10)?,
            error_message: row.get(11)?,
            added_at: row.get(12)?,
        })
    })?;

    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

async fn download_media_item(
    app_handle: tauri::AppHandle,
    ctx: &DownloadContext,
    item: &OfflineDownload,
) -> Result<(), JfgoatError> {
    update_download_status(ctx, &item.id, "Downloading", None)?;
    emit_download_progress(&app_handle, ctx, &item.id)?;

    // Get connection parameters directly
    let server_url = ctx.server_url.read().clone().ok_or_else(|| JfgoatError::Auth("No serverconnected".to_string()))?;
    let token = ctx.token.read().clone().ok_or_else(|| JfgoatError::Auth("No token".to_string()))?;

    pre_download_images(app_handle.clone(), ctx, &item.id, &server_url, &token).await;

    let download_dir = get_download_dir(app_handle.clone(), ctx)?;
    let _ = fs::create_dir_all(&download_dir);

    let url = format!(
        "{}/Videos/{}/stream?api_key={}&static=true&mediaSourceId={}",
        server_url.trim_end_matches('/'),
        item.id,
        token,
        item.id
    );

    let response = match ctx.http_client.get(&url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            let err_msg = format!("Request failed: {}", e);
            update_download_status(ctx, &item.id, "Failed", Some(&err_msg))?;
            emit_download_progress(&app_handle, ctx, &item.id)?;
            return Ok(());
        }
    };

    if !response.status().is_success() {
        let err_msg = format!("HTTP error: {}", response.status());
        update_download_status(ctx, &item.id, "Failed", Some(&err_msg))?;
        emit_download_progress(&app_handle, ctx, &item.id)?;
        return Ok(());
    }

    let total_bytes = response.content_length().unwrap_or(0);
    update_download_total_bytes(ctx, &item.id, total_bytes as i64)?;

    let ext = match response.headers().get(reqwest::header::CONTENT_TYPE) {
        Some(val) => {
            let content_type = val.to_str().unwrap_or("");
            if content_type.contains("x-matroska") || content_type.contains("mkv") {
                "mkv"
            } else if content_type.contains("webm") {
                "webm"
            } else if content_type.contains("quicktime") {
                "mov"
            } else {
                "mp4"
            }
        }
        None => "mp4",
    };

    let filename = format!("{}.{}", item.id, ext);
    let final_path = download_dir.join(&filename);
    let temp_path = download_dir.join(format!("{}.part", item.id));

    let mut file = match tokio::fs::File::create(&temp_path).await {
        Ok(f) => f,
        Err(e) => {
            let err_msg = format!("File create failed: {}", e);
            update_download_status(ctx, &item.id, "Failed", Some(&err_msg))?;
            emit_download_progress(&app_handle, ctx, &item.id)?;
            return Ok(());
        }
    };

    let mut stream = response.bytes_stream();
    let mut downloaded_bytes: i64 = 0;
    let mut last_db_update = std::time::Instant::now();
    let start_time = std::time::Instant::now();

    while let Some(chunk_result) = stream.next().await {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("Stream error: {}", e);
                update_download_status(ctx, &item.id, "Failed", Some(&err_msg))?;
                emit_download_progress(&app_handle, ctx, &item.id)?;
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Ok(());
            }
        };

        if let Err(e) = file.write_all(&chunk).await {
            let err_msg = format!("Write failed: {}", e);
            update_download_status(ctx, &item.id, "Failed", Some(&err_msg))?;
            emit_download_progress(&app_handle, ctx, &item.id)?;
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Ok(());
        }

        downloaded_bytes += chunk.len() as i64;

        let current_status = get_download_status_from_db(ctx, &item.id).unwrap_or_else(|_| "Downloading".to_string());

        if current_status == "Paused" {
            let _ = file.flush().await;
            println!("[download] Paused download for {}", item.id);
            return Ok(());
        } else if current_status == "Cancelled" {
            let _ = file.flush().await;
            drop(file);
            let _ = tokio::fs::remove_file(&temp_path).await;
            let _ = delete_download_from_db(ctx, &item.id);
            println!("[download] Cancelled download for {}", item.id);
            emit_download_progress(&app_handle, ctx, &item.id)?;
            return Ok(());
        }

        if last_db_update.elapsed() >= std::time::Duration::from_millis(500) {
            last_db_update = std::time::Instant::now();
            let elapsed_secs = start_time.elapsed().as_secs_f64();
            let speed = if elapsed_secs > 0.0 {
                downloaded_bytes as f64 / elapsed_secs
            } else {
                0.0
            };
            
            let progress = if total_bytes > 0 {
                (downloaded_bytes as f64 / total_bytes as f64) * 100.0
            } else {
                0.0
            };

            let _ = update_download_progress_in_db(
                ctx,
                &item.id,
                progress,
                downloaded_bytes,
                speed,
            );
            emit_download_progress(&app_handle, ctx, &item.id)?;
        }
    }

    if let Err(e) = file.flush().await {
        let err_msg = format!("Flush error: {}", e);
        update_download_status(ctx, &item.id, "Failed", Some(&err_msg))?;
        emit_download_progress(&app_handle, ctx, &item.id)?;
        return Ok(());
    }
    drop(file);

    if let Err(e) = tokio::fs::rename(&temp_path, &final_path).await {
        let err_msg = format!("Rename failed: {}", e);
        update_download_status(ctx, &item.id, "Failed", Some(&err_msg))?;
        emit_download_progress(&app_handle, ctx, &item.id)?;
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Ok(());
    }

    update_download_completed(ctx, &item.id, &final_path.to_string_lossy())?;
    emit_download_progress(&app_handle, ctx, &item.id)?;
    println!("[download] Completed download of {}", item.name);

    Ok(())
}

async fn pre_download_images(
    app_handle: tauri::AppHandle,
    ctx: &DownloadContext,
    item_id: &str,
    server_url: &str,
    token: &str,
) {
    let (image_tag, backdrop_tag, series_id) = {
        if let Ok(db) = ctx.db.read_conn() {
            db.query_row(
                "SELECT image_tag, backdrop_tag, series_id FROM media_items WHERE id = ?1",
                rusqlite::params![item_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0).ok().flatten(),
                        row.get::<_, Option<String>>(1).ok().flatten(),
                        row.get::<_, Option<String>>(2).ok().flatten(),
                    ))
                },
            )
            .unwrap_or((None, None, None))
        } else {
            (None, None, None)
        }
    };

    let cache_dir = match app_handle.path().app_cache_dir() {
        Ok(dir) => dir.join("image_cache"),
        Err(_) => return,
    };
    let _ = fs::create_dir_all(&cache_dir);

    if let Some(tag) = image_tag {
        let url = format!("{}/Items/{}/Images/Primary?maxWidth=400", server_url.trim_end_matches('/'), item_id);
        let path = cache_dir.join(format!("{}_{}.webp", item_id, tag));
        if !path.exists() {
            if let Ok(resp) = ctx.http_client.get(&url).header("X-Emby-Token", token).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        let _ = fs::write(path, bytes);
                    }
                }
            }
        }
    }

    if let Some(tag) = backdrop_tag {
        let url = format!("{}/Items/{}/Images/Backdrop?maxWidth=1280", server_url.trim_end_matches('/'), item_id);
        let path = cache_dir.join(format!("{}_{}.webp", item_id, tag));
        if !path.exists() {
            if let Ok(resp) = ctx.http_client.get(&url).header("X-Emby-Token", token).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        let _ = fs::write(path, bytes);
                    }
                }
            }
        }
    }

    if let Some(sid) = series_id {
        let series_image_tag = {
            if let Ok(db) = ctx.db.read_conn() {
                db.query_row(
                    "SELECT image_tag FROM media_items WHERE id = ?1",
                    rusqlite::params![sid],
                    |row| row.get::<_, Option<String>>(0),
                )
                .unwrap_or(None)
            } else {
                None
            }
        };

        if let Some(tag) = series_image_tag {
            let url = format!("{}/Items/{}/Images/Primary?maxWidth=400", server_url.trim_end_matches('/'), sid);
            let path = cache_dir.join(format!("{}_{}.webp", sid, tag));
            if !path.exists() {
                if let Ok(resp) = ctx.http_client.get(&url).header("X-Emby-Token", token).send().await {
                    if resp.status().is_success() {
                        if let Ok(bytes) = resp.bytes().await {
                            let _ = fs::write(path, bytes);
                        }
                    }
                }
            }
        }
    }
}

fn get_download_dir(app_handle: tauri::AppHandle, ctx: &DownloadContext) -> Result<PathBuf, JfgoatError> {
    let db = ctx.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let maybe_raw = db.query_row(
        "SELECT value FROM metadata WHERE key = 'user_preferences_v1'",
        [],
        |row| row.get::<_, String>(0),
    );

    let mut download_dir_pref = None;
    if let Ok(raw) = maybe_raw {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(dir) = parsed.get("download_directory") {
                if let Some(dir_str) = dir.as_str() {
                    if !dir_str.trim().is_empty() {
                        download_dir_pref = Some(PathBuf::from(dir_str));
                    }
                }
            }
        }
    }

    if let Some(dir) = download_dir_pref {
        Ok(dir)
    } else {
        let app_data = app_handle.path().app_data_dir().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        Ok(app_data.join("downloads"))
    }
}

fn update_download_status(ctx: &DownloadContext, item_id: &str, status: &str, error_message: Option<&str>) -> Result<(), JfgoatError> {
    let db = ctx.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    db.execute(
        "UPDATE offline_downloads SET status = ?1, error_message = ?2 WHERE id = ?3",
        rusqlite::params![status, error_message, item_id],
    ).map_err(|e| JfgoatError::Database(e.to_string()))?;
    Ok(())
}

fn update_download_total_bytes(ctx: &DownloadContext, item_id: &str, total_bytes: i64) -> Result<(), JfgoatError> {
    let db = ctx.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    db.execute(
        "UPDATE offline_downloads SET total_bytes = ?1 WHERE id = ?2",
        rusqlite::params![total_bytes, item_id],
    ).map_err(|e| JfgoatError::Database(e.to_string()))?;
    Ok(())
}

fn update_download_progress_in_db(ctx: &DownloadContext, item_id: &str, progress: f64, downloaded_bytes: i64, speed: f64) -> Result<(), JfgoatError> {
    let db = ctx.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    db.execute(
        "UPDATE offline_downloads
         SET progress = ?1, downloaded_bytes = ?2, speed_bytes_per_sec = ?3
         WHERE id = ?4",
        rusqlite::params![progress, downloaded_bytes, speed, item_id],
    ).map_err(|e| JfgoatError::Database(e.to_string()))?;
    Ok(())
}

fn update_download_completed(ctx: &DownloadContext, item_id: &str, local_path: &str) -> Result<(), JfgoatError> {
    let db = ctx.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    db.execute(
        "UPDATE offline_downloads SET status = 'Completed', progress = 100.0, local_path = ?1 WHERE id = ?2",
        rusqlite::params![local_path, item_id],
    ).map_err(|e| JfgoatError::Database(e.to_string()))?;
    Ok(())
}

fn get_download_status_from_db(ctx: &DownloadContext, item_id: &str) -> Result<String, JfgoatError> {
    let db = ctx.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let status: String = db.query_row(
        "SELECT status FROM offline_downloads WHERE id = ?1",
        rusqlite::params![item_id],
        |row| row.get(0),
    ).map_err(|e| JfgoatError::Database(e.to_string()))?;
    Ok(status)
}

fn delete_download_from_db(ctx: &DownloadContext, item_id: &str) -> Result<(), JfgoatError> {
    let db = ctx.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    db.execute(
        "DELETE FROM offline_downloads WHERE id = ?1",
        rusqlite::params![item_id],
    ).map_err(|e| JfgoatError::Database(e.to_string()))?;
    Ok(())
}

fn get_download_status_internal(ctx: &DownloadContext, item_id: &str) -> Result<Option<OfflineDownload>, JfgoatError> {
    let db = ctx.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let result = db.query_row(
        "SELECT id, server_id, user_id, name, type, local_path, status, progress,
                downloaded_bytes, total_bytes, speed_bytes_per_sec, error_message, added_at
         FROM offline_downloads
         WHERE id = ?1",
        rusqlite::params![item_id],
        |row| {
            Ok(OfflineDownload {
                id: row.get(0)?,
                server_id: row.get(1)?,
                user_id: row.get(2)?,
                name: row.get(3)?,
                item_type: row.get(4)?,
                local_path: row.get(5)?,
                status: row.get(6)?,
                progress: row.get(7)?,
                downloaded_bytes: row.get(8)?,
                total_bytes: row.get(9)?,
                speed_bytes_per_sec: row.get(10)?,
                error_message: row.get(11)?,
                added_at: row.get(12)?,
            })
        },
    );

    match result {
        Ok(item) => Ok(Some(item)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn emit_download_progress(app_handle: &tauri::AppHandle, ctx: &DownloadContext, item_id: &str) -> Result<(), JfgoatError> {
    if let Ok(Some(download)) = get_download_status_internal(ctx, item_id) {
        let _ = app_handle.emit("download-progress", download);
    } else {
        #[derive(Serialize, Clone)]
        struct DeletedDownloadPayload {
            id: String,
            status: String,
        }
        let _ = app_handle.emit("download-progress", DeletedDownloadPayload {
            id: item_id.to_string(),
            status: "Deleted".to_string(),
        });
    }
    Ok(())
}

fn build_ctx(state: &AppState) -> DownloadContext {
    DownloadContext {
        db: state.db.clone(),
        http_client: state.http_client.clone(),
        server_url: state.server_url.clone(),
        user_id: state.user_id.clone(),
        token: state.token.clone(),
        download_trigger: state.download_trigger.clone(),
    }
}

// ── Tauri Commands ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_download(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    item_id: String,
) -> Result<(), JfgoatError> {
    let (server_id, user_id) = {
        let server_id = state.get_server_id()?;
        let user_id = state.user_id.read().clone().ok_or_else(|| JfgoatError::Auth("No user ID".to_string()))?;
        (server_id, user_id)
    };

    let (name, item_type) = {
        let db = state.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        db.query_row(
            "SELECT name, type FROM media_items WHERE id = ?1 AND server_id = ?2 AND user_id = ?3",
            rusqlite::params![item_id, server_id, user_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ).map_err(|_| JfgoatError::Internal("Media item not found locally. Sync might be in progress.".to_string()))?
    };

    let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string();

    db.execute(
        "INSERT INTO offline_downloads (id, server_id, user_id, name, type, status, progress, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'Pending', 0.0, ?6)
         ON CONFLICT(id, server_id, user_id) DO UPDATE SET status = 'Pending', error_message = NULL",
        rusqlite::params![item_id, server_id, user_id, name, item_type, now_ms],
    ).map_err(|e| JfgoatError::Database(e.to_string()))?;

    let _ = state.download_trigger.send(());

    let ctx = build_ctx(&state);
    emit_download_progress(&app_handle, &ctx, &item_id)?;

    Ok(())
}

#[tauri::command]
pub async fn pause_download(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    item_id: String,
) -> Result<(), JfgoatError> {
    let ctx = build_ctx(&state);
    update_download_status(&ctx, &item_id, "Paused", None)?;
    emit_download_progress(&app_handle, &ctx, &item_id)?;
    Ok(())
}

#[tauri::command]
pub async fn resume_download(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    item_id: String,
) -> Result<(), JfgoatError> {
    let ctx = build_ctx(&state);
    update_download_status(&ctx, &item_id, "Pending", None)?;
    let _ = state.download_trigger.send(());
    emit_download_progress(&app_handle, &ctx, &item_id)?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_download(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    item_id: String,
) -> Result<(), JfgoatError> {
    let ctx = build_ctx(&state);
    let current_status = get_download_status_from_db(&ctx, &item_id).unwrap_or_default();
    if current_status == "Downloading" {
        update_download_status(&ctx, &item_id, "Cancelled", None)?;
    } else {
        delete_download_from_db(&ctx, &item_id)?;
        if let Ok(download_dir) = get_download_dir(app_handle.clone(), &ctx) {
            let temp_path = download_dir.join(format!("{}.part", item_id));
            let _ = std::fs::remove_file(temp_path);
        }
    }
    emit_download_progress(&app_handle, &ctx, &item_id)?;
    Ok(())
}

#[tauri::command]
pub async fn delete_download(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    item_id: String,
) -> Result<(), JfgoatError> {
    let ctx = build_ctx(&state);
    let local_path = {
        let db = state.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        db.query_row(
            "SELECT local_path FROM offline_downloads WHERE id = ?1",
            rusqlite::params![item_id],
            |row| row.get::<_, Option<String>>(0),
        ).unwrap_or(None)
    };

    if let Some(path_str) = local_path {
        let path = std::path::PathBuf::from(path_str);
        let _ = std::fs::remove_file(path);
    }

    delete_download_from_db(&ctx, &item_id)?;
    emit_download_progress(&app_handle, &ctx, &item_id)?;
    Ok(())
}

#[tauri::command]
pub async fn get_download_status(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<Option<OfflineDownload>, JfgoatError> {
    let ctx = build_ctx(&state);
    get_download_status_internal(&ctx, &item_id)
}

#[tauri::command]
pub async fn get_offline_downloads(
    state: State<'_, AppState>,
) -> Result<Vec<OfflineDownload>, JfgoatError> {
    let db = state.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let mut stmt = db.prepare(
        "SELECT id, server_id, user_id, name, type, local_path, status, progress,
                downloaded_bytes, total_bytes, speed_bytes_per_sec, error_message, added_at
         FROM offline_downloads
         ORDER BY added_at DESC"
    )?;
    
    let rows = stmt.query_map([], |row| {
        Ok(OfflineDownload {
            id: row.get(0)?,
            server_id: row.get(1)?,
            user_id: row.get(2)?,
            name: row.get(3)?,
            item_type: row.get(4)?,
            local_path: row.get(5)?,
            status: row.get(6)?,
            progress: row.get(7)?,
            downloaded_bytes: row.get(8)?,
            total_bytes: row.get(9)?,
            speed_bytes_per_sec: row.get(10)?,
            error_message: row.get(11)?,
            added_at: row.get(12)?,
        })
    })?;

    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}

#[tauri::command]
pub async fn select_download_directory() -> Result<Option<String>, JfgoatError> {
    if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
        Ok(Some(folder.path().to_string_lossy().into_owned()))
    } else {
        Ok(None)
    }
}
