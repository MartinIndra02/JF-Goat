use tauri::State;

use crate::api::media as media_api;
use crate::db::media::{
    MediaItem, get_offline_media_stream_cache, update_playback_ticks,
};
use crate::error::JfgoatError;
use crate::state::AppState;
use super::media_types::{StreamOption, MediaStreamInfo, ExternalUrl, ChapterInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackLifecycleEvent {
    Playing,
    Progress,
    Stopped,
}

impl PlaybackLifecycleEvent {
    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "playing" => Some(Self::Playing),
            "progress" => Some(Self::Progress),
            "stopped" => Some(Self::Stopped),
            _ => None,
        }
    }
}

/// Fetch media stream info (video quality, audio tracks, subtitles) for a media item.
#[tauri::command]
pub async fn get_media_streams(
    state: State<'_, AppState>,
    id: String,
) -> Result<MediaStreamInfo, JfgoatError> {
    if let Ok(db) = state.db.read_conn() {
        let cached = get_offline_media_stream_cache(&db, &id).ok();
        if let Some((streams_json_opt, subtitle_tracks_json, status_opt, local_path_opt)) = cached {
            if let Some(status) = status_opt {
                if let Some(streams_json) = streams_json_opt {
                    if !streams_json.is_empty() {
                        if let Ok(mut streams) = serde_json::from_str::<MediaStreamInfo>(&streams_json) {
                            if status == "Completed" {
                                streams.video_label = Some("Offline".to_string());
                                let downloaded_indices: Vec<i64> = subtitle_tracks_json
                                    .and_then(|s| serde_json::from_str(&s).ok())
                                    .unwrap_or_default();
                                streams.subtitle.retain(|track| {
                                    !track.is_external || downloaded_indices.contains(&track.index)
                                });
                            }
                            if streams.original_size.is_none() {
                                if let Ok(Some(download)) = crate::download::get_download_status_internal(&crate::download::build_ctx(&state), &id) {
                                    streams.original_size = Some(download.total_bytes);
                                }
                            }
                            return Ok(streams);
                        }
                    }
                }
            }
        }
    }

    let (jf_client, user_id, _) = state.get_jf_client()?;

    let res = media_api::fetch_item_media_streams(&jf_client, &user_id, &id).await?;
    let streams = res.media_streams;
    let original_size = res.media_sources.first().and_then(|src| src.size);

    let mut video = Vec::new();
    let mut audio = Vec::new();
    let mut subtitle = Vec::new();
    let mut video_label: Option<String> = None;

    for s in streams {
        let stream_type = s.stream_type.as_deref().unwrap_or("");
        let option = StreamOption {
            index: s.index.unwrap_or(0),
            codec: s.codec.clone().unwrap_or_default().to_uppercase(),
            display_title: s.display_title.clone().unwrap_or_default(),
            language: s.language.clone(),
            is_default: s.is_default.unwrap_or(false),
            delivery_method: s.delivery_method.clone(),
            is_external: s.is_external.unwrap_or(false),
            height: s.height,
            width: s.width,
            bit_rate: s.bit_rate,
            channels: s.channels,
            channel_layout: s.channel_layout.clone(),
            video_range: s.video_range.clone(),
            video_range_type: s.video_range_type.clone(),
        };

        match stream_type {
            "Video" => {
                if video_label.is_none() {
                    let resolution = match s.height.unwrap_or(0) {
                        h if h >= 2160 => "4K",
                        h if h >= 1080 => "HD",
                        h if h >= 720 => "HD",
                        h if h > 0 => "SD",
                        _ => "HD",
                    };
                    let range = s.video_range.as_deref().unwrap_or("SDR");
                    video_label = Some(format!("{} {}", resolution, range));
                }
                video.push(option);
            }
            "Audio" => audio.push(option),
            "Subtitle" => subtitle.push(option),
            _ => {}
        }
    }



    Ok(MediaStreamInfo {
        video,
        audio,
        subtitle,
        video_label,
        original_size,
    })
}

/// Fetch external URLs (IMDb, TMDB, TheTVDB, etc.) for a media item.
#[tauri::command]
pub async fn get_external_urls(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ExternalUrl>, JfgoatError> {
    let (jf_client, user_id, _) = state.get_jf_client()?;

    let urls = media_api::fetch_item_external_urls(&jf_client, &user_id, &id).await?;

    let result: Vec<ExternalUrl> = urls
        .into_iter()
        .filter_map(|u| {
            let name = u.name?;
            let url = u.url?;
            if url.is_empty() { return None; }
            Some(ExternalUrl { name, url })
        })
        .collect();

    Ok(result)
}

/// Fetch chapter markers for a media item.
#[tauri::command]
pub async fn get_item_chapters(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ChapterInfo>, JfgoatError> {
    let (jf_client, _, _) = state.get_jf_client()?;

    let chapters = media_api::fetch_item_chapters(&jf_client, &id).await?;

    let result = chapters
        .into_iter()
        .map(|chapter| ChapterInfo {
            name: chapter.name.unwrap_or_else(|| "Chapter".to_string()),
            start_ticks: chapter.start_position_ticks.unwrap_or(0),
            image_tag: chapter.image_tag,
            marker_type: chapter.marker_type,
            chapter_type: chapter.chapter_type,
        })
        .collect();

    Ok(result)
}

/// Toggle the played/unplayed state for a media item on the Jellyfin server
/// and update the local DB. Returns the new played state.
#[tauri::command]
pub async fn toggle_played(
    state: State<'_, AppState>,
    id: String,
    played: bool,
) -> Result<bool, JfgoatError> {
    let (jf_client, user_id, server_id) = state.get_jf_client()?;

    let new_played = !played;
    if new_played {
        media_api::mark_played(&jf_client, &user_id, &id).await?;
    } else {
        media_api::mark_unplayed(&jf_client, &user_id, &id).await?;
    }

    // Fetch the updated item from the server to get its new UserData
    let jf_item = media_api::fetch_item_by_id(&jf_client, &user_id, &id).await?;
    let item_type = jf_item.item_type.clone();
    let updated_item = MediaItem::from_jellyfin_item(jf_item, &server_id, &user_id);

    let mut items_to_update = vec![updated_item.clone()];

    if item_type == "Series" {
        if let Ok(children_resp) = media_api::fetch_series_children(&jf_client, &user_id, &id, 0, 500).await {
            for child in children_resp.items {
                items_to_update.push(MediaItem::from_jellyfin_item(child, &server_id, &user_id));
            }
        }
    } else if item_type == "Season" {
        if let Some(ref series_id) = updated_item.series_id {
            if let Ok(episodes_resp) = media_api::fetch_episodes(&jf_client, &user_id, series_id, &id).await {
                for ep in episodes_resp.items {
                    items_to_update.push(MediaItem::from_jellyfin_item(ep, &server_id, &user_id));
                }
            }
        }
    }

    // Update local DB in a single transaction
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        crate::db::media::insert_media_chunk(&db, &items_to_update)?;
    }

    Ok(new_played)
}

/// Toggle the favorite state for a media item on the Jellyfin server
/// and update the local DB. Returns the new favorite state.
#[tauri::command]
pub async fn toggle_favorite(
    state: State<'_, AppState>,
    id: String,
    is_favorite: bool,
) -> Result<bool, JfgoatError> {
    let (jf_client, user_id, server_id) = state.get_jf_client()?;

    let new_favorite = !is_favorite;
    if new_favorite {
        media_api::mark_favorite(&jf_client, &user_id, &id).await?;
    } else {
        media_api::unmark_favorite(&jf_client, &user_id, &id).await?;
    }

    // Fetch the updated item from the server to get its new UserData
    let jf_item = media_api::fetch_item_by_id(&jf_client, &user_id, &id).await?;
    let updated_item = MediaItem::from_jellyfin_item(jf_item, &server_id, &user_id);

    // Update local DB fully
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        crate::db::media::insert_media_chunk(&db, &[updated_item])?;
    }

    Ok(new_favorite)
}

/// Report playback lifecycle events to Jellyfin and keep local playback flags in sync.
pub async fn report_playback_lifecycle_internal(
    state: &AppState,
    item_id: &str,
    position_ticks: i64,
    duration_ticks: i64,
    event: PlaybackLifecycleEvent,
) -> Result<(), JfgoatError> {
    let (jf_client, user_id, server_id) = state.get_jf_client()?;

    let safe_position = position_ticks.max(0);
    let safe_duration = duration_ticks.max(0);

    let report_result = match event {
        PlaybackLifecycleEvent::Playing => {
            media_api::report_playback_playing(&jf_client, item_id, safe_position).await
        }
        PlaybackLifecycleEvent::Progress => {
            media_api::report_playback_progress(&jf_client, item_id, safe_position).await
        }
        PlaybackLifecycleEvent::Stopped => {
            media_api::report_playback_stopped(&jf_client, item_id, safe_position).await
        }
    };

    if let Err(e) = report_result {
        eprintln!("[playback] Failed to report playback lifecycle event to Jellyfin server: {:?}", e);
    }

    let near_end = if safe_duration > 0 {
        let remaining = (safe_duration - safe_position).max(0);
        let remaining_threshold = 60 * 10_000_000; // 60s
        let percent = safe_position as f64 / safe_duration as f64;
        percent >= 0.90 && (remaining <= remaining_threshold || percent >= 0.95)
    } else {
        false
    };

    if event == PlaybackLifecycleEvent::Stopped && near_end {
        if let Err(e) = media_api::mark_played(&jf_client, &user_id, &item_id).await {
            eprintln!("[playback] Failed to mark item played on Jellyfin server: {:?}", e);
        }
    }

    {
        let db = state
            .db
            .write_conn()
            .map_err(|e| JfgoatError::Internal(e.to_string()))?;

        if event == PlaybackLifecycleEvent::Stopped && near_end {
            update_playback_ticks(&db, item_id, &server_id, &user_id, 0, true)?;
        } else {
            update_playback_ticks(&db, item_id, &server_id, &user_id, safe_position, false)?;
        }
    }

    Ok(())
}

/// Report playback stop to Jellyfin and update local playback flags.
#[tauri::command]
pub async fn report_playback_stopped(
    state: State<'_, AppState>,
    item_id: String,
    position_ticks: i64,
    duration_ticks: i64,
) -> Result<(), JfgoatError> {
    report_playback_lifecycle_internal(
        &state,
        &item_id,
        position_ticks,
        duration_ticks,
        PlaybackLifecycleEvent::Stopped,
    )
    .await
}

fn get_friendly_language_name(code: &str) -> String {
    match code.to_lowercase().as_str() {
        "eng" | "en" => "English".to_string(),
        "spa" | "es" => "Spanish".to_string(),
        "fre" | "fra" | "fr" => "French".to_string(),
        "ger" | "deu" | "de" => "German".to_string(),
        "ita" | "it" => "Italian".to_string(),
        "jpn" | "ja" => "Japanese".to_string(),
        "chi" | "zho" | "zh" => "Chinese".to_string(),
        "rus" | "ru" => "Russian".to_string(),
        "por" | "pt" => "Portuguese".to_string(),
        "kor" | "ko" => "Korean".to_string(),
        "dut" | "nld" | "nl" => "Dutch".to_string(),
        "swe" | "sv" => "Swedish".to_string(),
        "nor" | "no" => "Norwegian".to_string(),
        "dan" | "da" => "Danish".to_string(),
        "fin" | "fi" => "Finnish".to_string(),
        "pol" | "pl" => "Polish".to_string(),
        "tur" | "tr" => "Turkish".to_string(),
        "ara" | "ar" => "Arabic".to_string(),
        "heb" | "he" => "Hebrew".to_string(),
        "cze" | "ces" | "cs" => "Czech".to_string(),
        "und" => "Unknown".to_string(),
        other => {
            if other.len() == 3 {
                other.to_uppercase()
            } else {
                format!("Track ({})", other)
            }
        }
    }
}
