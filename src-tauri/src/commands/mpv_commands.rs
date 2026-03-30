use tauri::State;

use crate::api::media as media_api;
use crate::commands::media_commands::{
    report_playback_lifecycle_internal, PlaybackLifecycleEvent,
};
use crate::error::JfgoatError;
use crate::mpv::{MpvCommand, MpvState};
use crate::state::AppState;

#[cfg(target_os = "windows")]
use crate::mpv::{hide_mpv_window, show_mpv_window};

fn build_playback_url_with_options(
    state: &AppState,
    item_id: &str,
    audio_stream_index: Option<i64>,
    subtitle_stream_index: Option<i64>,
    max_streaming_bitrate: Option<i64>,
    target_height: Option<i64>,
) -> Result<String, JfgoatError> {
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

    let payload = media_api::build_playback_config_payload(
        &server_url,
        &token,
        item_id,
        audio_stream_index,
        subtitle_stream_index,
        max_streaming_bitrate,
        target_height,
    );

    Ok(payload.url().to_string())
}

#[tauri::command]
pub fn mpv_play(
    mpv: State<'_, MpvState>,
    app_state: State<'_, AppState>,
    item_id: String,
    start_ticks: i64,
    audio_stream_index: Option<i64>,
    subtitle_stream_index: Option<i64>,
    max_streaming_bitrate: Option<i64>,
    target_height: Option<i64>,
) -> Result<(), JfgoatError> {
    let url = build_playback_url_with_options(
        &app_state,
        &item_id,
        audio_stream_index,
        subtitle_stream_index,
        max_streaming_bitrate,
        target_height,
    )?;
    let start_seconds = start_ticks as f64 / 10_000_000.0;

    #[cfg(target_os = "windows")]
    show_mpv_window(mpv.child_hwnd);

    mpv.cmd_tx
        .send(MpvCommand::LoadFile {
            url,
            start_seconds,
            // Stream selection is applied via the Jellyfin URL query parameters.
            // Avoid setting mpv aid/sid directly because those are runtime-local IDs.
            audio_track: None,
            subtitle_track: None,
        })
        .map_err(|e| JfgoatError::Internal(format!("mpv send failed: {}", e)))?;

    Ok(())
}

#[tauri::command]
pub async fn report_playback_lifecycle(
    state: State<'_, AppState>,
    item_id: String,
    position_ticks: i64,
    duration_ticks: i64,
    event: String,
) -> Result<(), JfgoatError> {
    let event = PlaybackLifecycleEvent::from_wire(event.as_str()).ok_or_else(|| {
        JfgoatError::Internal(format!("Invalid playback lifecycle event: {}", event))
    })?;

    report_playback_lifecycle_internal(
        &state,
        &item_id,
        position_ticks,
        duration_ticks,
        event,
    )
    .await
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
pub fn mpv_set_mute(mpv: State<'_, MpvState>, muted: bool) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::SetMute(muted))
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn mpv_set_playback_rate(mpv: State<'_, MpvState>, rate: f64) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::SetRate(rate))
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn mpv_set_subtitle_position(
    mpv: State<'_, MpvState>,
    position: i64,
) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::SetSubtitlePosition(position))
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn mpv_set_video_scale(mpv: State<'_, MpvState>, mode: String) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::SetVideoScale(mode))
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn mpv_set_audio_track(mpv: State<'_, MpvState>, track: i64) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::SetAudioTrack(track))
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn mpv_set_subtitle_track(
    mpv: State<'_, MpvState>,
    track: Option<i64>,
) -> Result<(), JfgoatError> {
    mpv.cmd_tx
        .send(MpvCommand::SetSubtitleTrack(track))
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
