use tauri::State;

use crate::api::client::JellyfinClient;
use crate::api::media as media_api;
use crate::commands::media_commands::{
    report_playback_lifecycle_internal, PlaybackLifecycleEvent,
};
use crate::error::JfgoatError;
use crate::mpv::{MpvCommand, MpvState};
use crate::state::AppState;

#[cfg(target_os = "windows")]
use crate::mpv::{hide_mpv_window, show_mpv_window};

fn to_absolute_url(server_url: &str, raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }

    let server_base = server_url.trim_end_matches('/');
    if trimmed.starts_with('/') {
        format!("{}{}", server_base, trimmed)
    } else {
        format!("{}/{}", server_base, trimmed)
    }
}

fn append_api_key_query(url: &str, api_key: &str) -> String {
    if url.contains("api_key=") {
        return url.to_string();
    }

    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{}{}api_key={}", url, separator, urlencoding::encode(api_key))
}

fn read_playback_connection_params(
    state: &AppState,
) -> Result<(String, String, String, String), JfgoatError> {
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

fn resolve_stream_url_from_playback_context(
    server_url: &str,
    api_key: &str,
    fallback_payload: &media_api::PlaybackConfigPayload,
    playback_info: &media_api::JellyfinPlaybackInfoResponse,
    prefer_transcode: bool,
) -> Option<String> {
    let source = playback_info.media_sources.first()?;

    let play_method = source
        .play_method
        .as_deref()
        .and_then(media_api::PlayMethod::from_wire);

    let direct_stream_url = source
        .direct_stream_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| append_api_key_query(&to_absolute_url(server_url, value), api_key));

    let transcode_stream_url = source
        .transcoding_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| append_api_key_query(&to_absolute_url(server_url, value), api_key));

    if prefer_transcode {
        if let Some(url) = transcode_stream_url.as_ref() {
            return Some(url.clone());
        }
    }

    match play_method {
        Some(media_api::PlayMethod::DirectPlay) => {
            return Some(fallback_payload.url().to_string());
        }
        Some(media_api::PlayMethod::DirectStream) => {
            if let Some(url) = direct_stream_url.as_ref() {
                return Some(url.clone());
            }
            return Some(fallback_payload.url().to_string());
        }
        Some(media_api::PlayMethod::Transcode) => {
            if let Some(url) = transcode_stream_url.as_ref() {
                return Some(url.clone());
            }
        }
        None => {}
    }

    if source.supports_direct_play.unwrap_or(false) {
        return Some(fallback_payload.url().to_string());
    }

    if source.supports_direct_stream.unwrap_or(false) {
        if let Some(url) = direct_stream_url {
            return Some(url);
        }
        return Some(fallback_payload.url().to_string());
    }

    if source.supports_transcoding.unwrap_or(false) {
        if let Some(url) = transcode_stream_url {
            return Some(url);
        }
    }

    None
}

async fn build_playback_url_with_options(
    state: &AppState,
    item_id: &str,
    audio_stream_index: Option<i64>,
    subtitle_stream_index: Option<i64>,
    max_streaming_bitrate: Option<i64>,
    target_height: Option<i64>,
) -> Result<String, JfgoatError> {
    let (server_url, token, user_id, device_id) = read_playback_connection_params(state)?;

    let payload = media_api::build_playback_config_payload(
        &server_url,
        &token,
        item_id,
        audio_stream_index,
        subtitle_stream_index,
        max_streaming_bitrate,
        target_height,
    );

    let prefer_transcode =
        max_streaming_bitrate.unwrap_or(0) > 0 || target_height.unwrap_or(0) > 0;

    let playback_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let resolved_from_context = match media_api::fetch_playback_info(
        &playback_client,
        &user_id,
        item_id,
        audio_stream_index,
        subtitle_stream_index,
        max_streaming_bitrate,
        target_height,
    )
    .await
    {
        Ok(playback_info) => resolve_stream_url_from_playback_context(
            &server_url,
            &token,
            &payload,
            &playback_info,
            prefer_transcode,
        ),
        Err(err) => {
            eprintln!(
                "[mpv] PlaybackInfo lookup failed for {}. Falling back to default stream URL: {}",
                item_id, err
            );
            None
        }
    };

    if let Some(url) = resolved_from_context {
        return Ok(url);
    }

    Ok(payload.url().to_string())
}

#[tauri::command]
pub async fn mpv_play(
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
    )
    .await?;
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
