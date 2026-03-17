use tauri::State;

use crate::error::JfgoatError;
use crate::mpv::{MpvCommand, MpvState};
use crate::state::AppState;

#[cfg(target_os = "windows")]
use crate::mpv::{hide_mpv_window, show_mpv_window};

/// Build the Jellyfin direct-play URL for a media item.
fn build_playback_url(state: &AppState, item_id: &str) -> Result<String, JfgoatError> {
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
        .ok_or_else(|| JfgoatError::Auth("No token available".to_string()))?;

    Ok(format!(
        "{}/Videos/{}/stream?static=true&mediaSourceId={}&api_key={}",
        server_url.trim_end_matches('/'),
        item_id,
        item_id,
        token
    ))
}

#[tauri::command]
pub fn mpv_play(
    mpv: State<'_, MpvState>,
    app_state: State<'_, AppState>,
    item_id: String,
    start_ticks: i64,
) -> Result<(), JfgoatError> {
    let url = build_playback_url(&app_state, &item_id)?;
    let start_seconds = start_ticks as f64 / 10_000_000.0;

    #[cfg(target_os = "windows")]
    show_mpv_window(mpv.child_hwnd);

    mpv.cmd_tx
        .send(MpvCommand::LoadFile { url, start_seconds })
        .map_err(|e| JfgoatError::Internal(format!("mpv send failed: {}", e)))?;

    Ok(())
}

#[tauri::command]
pub fn mpv_toggle_pause(mpv: State<'_, MpvState>) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::TogglePause)
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn mpv_seek(mpv: State<'_, MpvState>, seconds: f64) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::SeekRelative(seconds))
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn mpv_seek_absolute(mpv: State<'_, MpvState>, seconds: f64) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::SeekAbsolute(seconds))
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn mpv_set_volume(mpv: State<'_, MpvState>, volume: f64) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::SetVolume(volume))
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn mpv_stop(mpv: State<'_, MpvState>) -> Result<(), JfgoatError> {
    #[cfg(target_os = "windows")]
    hide_mpv_window(mpv.child_hwnd);

    mpv.cmd_tx
        .send(MpvCommand::Stop)
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}
