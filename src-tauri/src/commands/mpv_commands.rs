use tauri::State;
use urlencoding::encode;

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

    let should_transcode = max_streaming_bitrate.unwrap_or(0) > 0 || target_height.unwrap_or(0) > 0;
    let mut query_params = vec![
        ("api_key".to_string(), token),
        ("static".to_string(), if should_transcode { "false" } else { "true" }.to_string()),
    ];

    if should_transcode {
        if let Some(idx) = audio_stream_index {
            if idx >= 0 {
                query_params.push(("AudioStreamIndex".to_string(), idx.to_string()));
            }
        }

        if let Some(idx) = subtitle_stream_index {
            if idx >= 0 {
                query_params.push(("SubtitleStreamIndex".to_string(), idx.to_string()));
            } else {
                query_params.push(("SubtitleStreamIndex".to_string(), "-1".to_string()));
            }
        }
    } else {
        // Keep direct-play requests as lightweight as possible.
        query_params.push(("mediaSourceId".to_string(), item_id.to_string()));
    }

    if let Some(bitrate) = max_streaming_bitrate {
        if bitrate > 0 {
            query_params.push(("MaxStreamingBitrate".to_string(), bitrate.to_string()));
        }
    }

    if let Some(height) = target_height {
        if height > 0 {
            query_params.push(("MaxHeight".to_string(), height.to_string()));
        }
    }

    let query = query_params
        .into_iter()
        .map(|(k, v)| format!("{}={}", encode(&k), encode(&v)))
        .collect::<Vec<_>>()
        .join("&");

    Ok(format!(
        "{}/Videos/{}/stream?{}",
        server_url.trim_end_matches('/'),
        item_id,
        query
    ))
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
