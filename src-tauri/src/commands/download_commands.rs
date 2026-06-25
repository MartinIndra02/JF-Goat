use tauri::State;

use crate::error::JfgoatError;
use crate::state::AppState;
use crate::download::{
    OfflineDownload, build_ctx, emit_download_progress, update_download_status,
    get_download_status_from_db, delete_download_from_db, get_download_dir,
    get_download_status_internal
};

// ── Tauri Commands ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_download(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    item_id: String,
    audio_tracks: Option<Vec<i64>>,
    subtitle_tracks: Option<Vec<i64>>,
    transcode_height: Option<i64>,
    transcode_bitrate: Option<i64>,
    media_streams_json: Option<String>,
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

    let audio_tracks_json = audio_tracks.map(|v| serde_json::to_string(&v).ok()).flatten();
    let subtitle_tracks_json = subtitle_tracks.map(|v| serde_json::to_string(&v).ok()).flatten();

    db.execute(
        "INSERT INTO offline_downloads (id, server_id, user_id, name, type, status, progress, added_at, audio_tracks, subtitle_tracks, transcode_height, transcode_bitrate, media_streams_json)
         VALUES (?1, ?2, ?3, ?4, ?5, 'Pending', 0.0, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id, server_id, user_id) DO UPDATE SET
            status = 'Pending',
            error_message = NULL,
            audio_tracks = ?7,
            subtitle_tracks = ?8,
            transcode_height = ?9,
            transcode_bitrate = ?10,
            media_streams_json = ?11,
            added_at = ?6",
        rusqlite::params![
            item_id,
            server_id,
            user_id,
            name,
            item_type,
            now_ms,
            audio_tracks_json,
            subtitle_tracks_json,
            transcode_height,
            transcode_bitrate,
            media_streams_json,
        ],
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
            
            // Clean up downloaded subtitles
            if let Ok(entries) = std::fs::read_dir(&download_dir) {
                for entry in entries.filter_map(Result::ok) {
                    let p = entry.path();
                    if p.is_file() {
                        if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                            if fname.starts_with(&item_id) {
                                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                                if ext == "srt" || ext == "vtt" || ext == "ass" || ext == "sub" {
                                    let _ = std::fs::remove_file(p);
                                }
                            }
                        }
                    }
                }
            }
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
        let _ = std::fs::remove_file(&path);
        
        // Clean up downloaded subtitles in same directory
        if let Some(parent) = path.parent() {
            if let Ok(entries) = std::fs::read_dir(parent) {
                for entry in entries.filter_map(Result::ok) {
                    let p = entry.path();
                    if p.is_file() {
                        if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                            if fname.starts_with(&item_id) {
                                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                                if ext == "srt" || ext == "vtt" || ext == "ass" || ext == "sub" {
                                    let _ = std::fs::remove_file(p);
                                }
                            }
                        }
                    }
                }
            }
        }
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
                downloaded_bytes, total_bytes, speed_bytes_per_sec, error_message, added_at,
                audio_tracks, subtitle_tracks, transcode_height, transcode_bitrate
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
            audio_tracks: row.get(13)?,
            subtitle_tracks: row.get(14)?,
            transcode_height: row.get(15)?,
            transcode_bitrate: row.get(16)?,
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
