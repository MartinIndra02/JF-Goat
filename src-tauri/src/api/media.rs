use serde::{Deserialize, Serialize};
use urlencoding::encode;

use crate::api::client::JellyfinClient;
use crate::error::JfgoatError;

#[derive(Debug, Deserialize)]
pub struct JellyfinItemsResponse {
    #[serde(alias = "Items", default)]
    pub items: Vec<JellyfinItem>,
    #[serde(alias = "TotalRecordCount", default)]
    pub total_record_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinViewsResponse {
    #[serde(alias = "Items", default)]
    pub items: Vec<JellyfinView>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinView {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name", default)]
    pub name: Option<String>,
    #[serde(alias = "CollectionType", default)]
    pub collection_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinItem {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name", default)]
    pub name: Option<String>,
    #[serde(alias = "Type", default)]
    pub item_type: String,
    #[serde(alias = "ParentId")]
    pub parent_id: Option<String>,
    #[serde(alias = "SeriesId")]
    pub series_id: Option<String>,
    #[serde(alias = "SeriesName")]
    pub series_name: Option<String>,
    #[serde(alias = "SeasonId")]
    pub season_id: Option<String>,
    #[serde(alias = "SeasonName")]
    pub season_name: Option<String>,
    #[serde(alias = "IndexNumber")]
    pub index_number: Option<i64>,
    #[serde(alias = "ProductionYear")]
    pub production_year: Option<i64>,
    #[serde(alias = "Overview")]
    pub overview: Option<String>,
    #[serde(alias = "ImageTags")]
    pub image_tags: Option<ImageTags>,
    #[serde(alias = "BackdropImageTags")]
    pub backdrop_image_tags: Option<Vec<String>>,
    #[serde(alias = "DateCreated")]
    pub date_created: Option<String>,
    #[serde(alias = "DateLastMediaAdded", default)]
    pub date_last_media_added: Option<String>,
    #[serde(alias = "PremiereDate", default)]
    pub premiere_date: Option<String>,
    #[serde(alias = "CommunityRating")]
    pub community_rating: Option<f64>,
    #[serde(alias = "OfficialRating")]
    pub official_rating: Option<String>,
    #[serde(alias = "Genres")]
    pub genres: Option<Vec<String>>,
    #[serde(alias = "RunTimeTicks")]
    pub run_time_ticks: Option<i64>,
    #[serde(alias = "UserData")]
    pub user_data: Option<JellyfinUserData>,
}

#[derive(Debug, Deserialize)]
pub struct ImageTags {
    #[serde(alias = "Primary")]
    pub primary: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinUserData {
    #[serde(alias = "Played")]
    pub played: Option<bool>,
    #[serde(alias = "PlayCount")]
    pub play_count: Option<i64>,
    #[serde(alias = "PlaybackPositionTicks")]
    pub playback_position_ticks: Option<i64>,
    #[serde(alias = "IsFavorite")]
    pub is_favorite: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total_record_count: u32,
    pub start_index: u32,
    pub limit: u32,
    pub has_more: bool,
}

impl<T> PaginatedResult<T> {
    pub fn from_known_total(
        items: Vec<T>,
        total_record_count: u32,
        start_index: u32,
        limit: u32,
    ) -> Self {
        let has_more = start_index.saturating_add(items.len() as u32) < total_record_count;
        Self {
            items,
            total_record_count,
            start_index,
            limit,
            has_more,
        }
    }

    pub fn from_page_len(items: Vec<T>, start_index: u32, limit: u32) -> Self {
        let total_record_count = start_index.saturating_add(items.len() as u32);
        let has_more = limit > 0 && (items.len() as u32) >= limit;
        Self {
            items,
            total_record_count,
            start_index,
            limit,
            has_more,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DirectPlaybackQuery {
    pub api_key: String,
    #[serde(rename = "static")]
    pub static_mode: String,
    #[serde(rename = "mediaSourceId")]
    pub media_source_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranscodePlaybackQuery {
    pub api_key: String,
    #[serde(rename = "static")]
    pub static_mode: String,
    #[serde(rename = "AudioStreamIndex", skip_serializing_if = "Option::is_none")]
    pub audio_stream_index: Option<i64>,
    #[serde(rename = "SubtitleStreamIndex", skip_serializing_if = "Option::is_none")]
    pub subtitle_stream_index: Option<i64>,
    #[serde(rename = "MaxStreamingBitrate", skip_serializing_if = "Option::is_none")]
    pub max_streaming_bitrate: Option<i64>,
    #[serde(rename = "MaxHeight", skip_serializing_if = "Option::is_none")]
    pub max_height: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum PlaybackConfigPayload {
    Direct {
        item_id: String,
        endpoint: String,
        url: String,
        query: DirectPlaybackQuery,
    },
    Transcode {
        item_id: String,
        endpoint: String,
        url: String,
        query: TranscodePlaybackQuery,
    },
}

impl PlaybackConfigPayload {
    pub fn url(&self) -> &str {
        match self {
            Self::Direct { url, .. } | Self::Transcode { url, .. } => url,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayMethod {
    DirectPlay,
    DirectStream,
    Transcode,
}

impl PlayMethod {
    pub fn from_wire(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "directplay" => Some(Self::DirectPlay),
            "directstream" => Some(Self::DirectStream),
            "transcode" => Some(Self::Transcode),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct JellyfinPlaybackInfoResponse {
    #[serde(alias = "MediaSources", default)]
    pub media_sources: Vec<JellyfinPlaybackMediaSource>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JellyfinPlaybackMediaSource {
    #[serde(alias = "PlayMethod", default)]
    pub play_method: Option<String>,
    #[serde(alias = "DirectStreamUrl", default)]
    pub direct_stream_url: Option<String>,
    #[serde(alias = "TranscodingUrl", default)]
    pub transcoding_url: Option<String>,
    #[serde(alias = "SupportsDirectPlay", default)]
    pub supports_direct_play: Option<bool>,
    #[serde(alias = "SupportsDirectStream", default)]
    pub supports_direct_stream: Option<bool>,
    #[serde(alias = "SupportsTranscoding", default)]
    pub supports_transcoding: Option<bool>,
}

fn build_query_string(query_params: &[(String, String)]) -> String {
    query_params
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn build_playback_config_payload(
    server_url: &str,
    token: &str,
    item_id: &str,
    audio_stream_index: Option<i64>,
    subtitle_stream_index: Option<i64>,
    max_streaming_bitrate: Option<i64>,
    target_height: Option<i64>,
) -> PlaybackConfigPayload {
    let endpoint = format!("/Videos/{}/stream", item_id);
    let server_base = server_url.trim_end_matches('/');
    let should_transcode =
        max_streaming_bitrate.unwrap_or(0) > 0 || target_height.unwrap_or(0) > 0;

    if should_transcode {
        let mut query = TranscodePlaybackQuery {
            api_key: token.to_string(),
            static_mode: "false".to_string(),
            audio_stream_index: None,
            subtitle_stream_index: None,
            max_streaming_bitrate: None,
            max_height: None,
        };

        if let Some(idx) = audio_stream_index {
            if idx >= 0 {
                query.audio_stream_index = Some(idx);
            }
        }

        if let Some(idx) = subtitle_stream_index {
            query.subtitle_stream_index = Some(if idx >= 0 { idx } else { -1 });
        }

        if let Some(bitrate) = max_streaming_bitrate {
            if bitrate > 0 {
                query.max_streaming_bitrate = Some(bitrate);
            }
        }

        if let Some(height) = target_height {
            if height > 0 {
                query.max_height = Some(height);
            }
        }

        let mut query_params = vec![
            ("api_key".to_string(), query.api_key.clone()),
            ("static".to_string(), query.static_mode.clone()),
        ];
        if let Some(v) = query.audio_stream_index {
            query_params.push(("AudioStreamIndex".to_string(), v.to_string()));
        }
        if let Some(v) = query.subtitle_stream_index {
            query_params.push(("SubtitleStreamIndex".to_string(), v.to_string()));
        }
        if let Some(v) = query.max_streaming_bitrate {
            query_params.push(("MaxStreamingBitrate".to_string(), v.to_string()));
        }
        if let Some(v) = query.max_height {
            query_params.push(("MaxHeight".to_string(), v.to_string()));
        }

        let url = format!("{}{}?{}", server_base, endpoint, build_query_string(&query_params));

        return PlaybackConfigPayload::Transcode {
            item_id: item_id.to_string(),
            endpoint,
            url,
            query,
        };
    }

    let query = DirectPlaybackQuery {
        api_key: token.to_string(),
        static_mode: "true".to_string(),
        media_source_id: item_id.to_string(),
    };
    let query_params = vec![
        ("api_key".to_string(), query.api_key.clone()),
        ("static".to_string(), query.static_mode.clone()),
        ("mediaSourceId".to_string(), query.media_source_id.clone()),
    ];
    let url = format!("{}{}?{}", server_base, endpoint, build_query_string(&query_params));

    PlaybackConfigPayload::Direct {
        item_id: item_id.to_string(),
        endpoint,
        url,
        query,
    }
}

/// Fetch playback context from Jellyfin to discover the effective play method.
pub async fn fetch_playback_info(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
    audio_stream_index: Option<i64>,
    subtitle_stream_index: Option<i64>,
    max_streaming_bitrate: Option<i64>,
    target_height: Option<i64>,
) -> Result<JellyfinPlaybackInfoResponse, JfgoatError> {
    let path = format!(
        "/Items/{}/PlaybackInfo?UserId={}",
        item_id,
        encode(user_id)
    );

    let mut body = serde_json::Map::new();
    body.insert(
        "UserId".to_string(),
        serde_json::Value::String(user_id.to_string()),
    );
    body.insert(
        "StartTimeTicks".to_string(),
        serde_json::Value::Number(serde_json::Number::from(0)),
    );
    body.insert("IsPlayback".to_string(), serde_json::Value::Bool(true));
    body.insert("AutoOpenLiveStream".to_string(), serde_json::Value::Bool(true));

    if let Some(index) = audio_stream_index {
        if index >= 0 {
            body.insert(
                "AudioStreamIndex".to_string(),
                serde_json::Value::Number(serde_json::Number::from(index)),
            );
        }
    }

    if let Some(index) = subtitle_stream_index {
        let normalized = if index >= 0 { index } else { -1 };
        body.insert(
            "SubtitleStreamIndex".to_string(),
            serde_json::Value::Number(serde_json::Number::from(normalized)),
        );
    }

    if let Some(bitrate) = max_streaming_bitrate {
        if bitrate > 0 {
            body.insert(
                "MaxStreamingBitrate".to_string(),
                serde_json::Value::Number(serde_json::Number::from(bitrate)),
            );
        }
    }

    if let Some(height) = target_height {
        if height > 0 {
            body.insert(
                "MaxHeight".to_string(),
                serde_json::Value::Number(serde_json::Number::from(height)),
            );
        }
    }

    let resp = client
        .post_json(&path, &serde_json::Value::Object(body))
        .await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch playback info for {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let data: JellyfinPlaybackInfoResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse playback info response: {}", e))
    })?;

    Ok(data)
}

/// Fetch the user's top-level libraries (Views).
pub async fn fetch_user_views(
    client: &JellyfinClient,
    user_id: &str,
) -> Result<JellyfinViewsResponse, JfgoatError> {
    let path = format!("/Users/{}/Views", user_id);
    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch views: status {}",
            resp.status()
        )));
    }

    let data: JellyfinViewsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse views response: {}", e))
    })?;

    Ok(data)
}

/// Fetch items scoped to a specific library view (with total count).
/// Use for the first request per view to get the total record count.
pub async fn fetch_view_items(
    client: &JellyfinClient,
    user_id: &str,
    view_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    fetch_view_items_inner(client, user_id, view_id, start_index, limit, true).await
}

/// Fetch items scoped to a specific library view (without total count).
/// Use for all subsequent pagination requests after the first per view.
pub async fn fetch_view_items_no_count(
    client: &JellyfinClient,
    user_id: &str,
    view_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    fetch_view_items_inner(client, user_id, view_id, start_index, limit, false).await
}

pub async fn fetch_view_items_paginated(
    client: &JellyfinClient,
    user_id: &str,
    view_id: &str,
    start_index: u32,
    limit: u32,
    enable_total_count: bool,
) -> Result<PaginatedResult<JellyfinItem>, JfgoatError> {
    let response =
        fetch_view_items_inner(client, user_id, view_id, start_index, limit, enable_total_count)
            .await?;

    if enable_total_count {
        return Ok(PaginatedResult::from_known_total(
            response.items,
            response.total_record_count,
            start_index,
            limit,
        ));
    }

    Ok(PaginatedResult::from_page_len(
        response.items,
        start_index,
        limit,
    ))
}

/// Fetch a paginated list of user-scoped items with UserData only.
/// Used by the incremental user data refresh loop after initial sync.
pub async fn fetch_user_items_userdata(
    client: &JellyfinClient,
    user_id: &str,
    start_index: u32,
    limit: u32,
    enable_total_count: bool,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?StartIndex={}&Limit={}&Recursive=true\
         &IncludeItemTypes=Movie,Series,Episode,Season,BoxSet,MusicAlbum,MusicArtist,Audio\
         &Fields=UserData\
         &EnableTotalRecordCount={}",
        user_id, start_index, limit, enable_total_count
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch user data items: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse user data items response: {}", e))
    })?;

    Ok(data)
}

async fn fetch_view_items_inner(
    client: &JellyfinClient,
    user_id: &str,
    view_id: &str,
    start_index: u32,
    limit: u32,
    enable_total_count: bool,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?ParentId={}&StartIndex={}&Limit={}&Recursive=true\
         &IncludeItemTypes=Movie,Series,Episode,Season,BoxSet,MusicAlbum,MusicArtist,Audio\
         &Fields=Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &EnableTotalRecordCount={}",
        user_id, view_id, start_index, limit, enable_total_count
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch items: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse items response: {}", e))
    })?;

    Ok(data)
}

/// Fetch only Series items for a TV Shows library view (with total count).
pub async fn fetch_series(
    client: &JellyfinClient,
    user_id: &str,
    view_id: &str,
    start_index: u32,
    limit: u32,
    enable_total_count: bool,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?ParentId={}&StartIndex={}&Limit={}&Recursive=false\
         &IncludeItemTypes=Series\
         &Fields=Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &EnableTotalRecordCount={}",
        user_id, view_id, start_index, limit, enable_total_count
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch series: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse series response: {}", e))
    })?;

    Ok(data)
}

/// Fetch Seasons and Episodes for a specific Series.
pub async fn fetch_series_children(
    client: &JellyfinClient,
    user_id: &str,
    series_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?ParentId={}&StartIndex={}&Limit={}&Recursive=true\
         &IncludeItemTypes=Season,Episode\
         &Fields=Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &EnableTotalRecordCount=false",
        user_id, series_id, start_index, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch series children: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse series children response: {}", e))
    })?;

    Ok(data)
}

/// Fetch the absolute total item count across the user's entire library.
/// Uses Limit=1 so the response body is minimal; we only care about TotalRecordCount.
pub async fn fetch_total_item_count(
    client: &JellyfinClient,
    user_id: &str,
) -> Result<u32, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?Limit=1&Recursive=true&EnableTotalRecordCount=true",
        user_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch total item count: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse total item count response: {}", e))
    })?;

    Ok(data.total_record_count)
}

/// Fetch items the user is currently watching (have playback progress).
/// Uses the Jellyfin `/Users/{id}/Items/Resume` endpoint.
pub async fn fetch_resume_items(
    client: &JellyfinClient,
    user_id: &str,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/Resume?Limit={}&Recursive=true\
         &Fields=Overview,Genres,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &IncludeItemTypes=Movie,Episode\
         &EnableTotalRecordCount=false\
         &MediaTypes=Video",
        user_id, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch resume items: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse resume items response: {}", e))
    })?;

    Ok(data)
}

/// Fetch the next episodes to watch across all series.
/// Uses the Jellyfin `/Shows/NextUp` endpoint.
pub async fn fetch_next_up(
    client: &JellyfinClient,
    user_id: &str,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Shows/NextUp?UserId={}&Limit={}\
         &Fields=Overview,Genres,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &EnableTotalRecordCount=false",
        user_id, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch next up: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse next up response: {}", e))
    })?;

    Ok(data)
}

/// Fetch the latest items added to a specific library view.
/// Uses the Jellyfin `/Users/{id}/Items/Latest` endpoint.
pub async fn fetch_latest_items(
    client: &JellyfinClient,
    user_id: &str,
    parent_id: &str,
    limit: u32,
) -> Result<Vec<JellyfinItem>, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/Latest?ParentId={}&Limit={}\
         &Fields=Overview,Genres,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &GroupItems=true",
        user_id, parent_id, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch latest items: status {}",
            resp.status()
        )));
    }

    // The /Latest endpoint returns a flat array of items, not wrapped in { Items: [...] }
    let data: Vec<JellyfinItem> = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse latest items response: {}", e))
    })?;

    Ok(data)
}

/// Fetch a single item by ID from the Jellyfin server.
/// Used as fallback when item isn't in local DB (pre-sync).
pub async fn fetch_item_by_id(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<JellyfinItem, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/{}?Fields=Overview,Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags,UserData",
        user_id, item_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch item {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let text = resp.text().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to read item response body: {}", e))
    })?;

    serde_json::from_str::<JellyfinItem>(&text).map_err(|e| {
        let snippet = &text[..text.len().min(300)];
        JfgoatError::Http(format!("Failed to parse item {}: {} | body: {}", item_id, e, snippet))
    })
}

/// Fetch seasons for a series from the Jellyfin server.
/// Used as fallback when seasons aren't in local DB (pre-sync).
pub async fn fetch_seasons(
    client: &JellyfinClient,
    user_id: &str,
    series_id: &str,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Shows/{}/Seasons?UserId={}&Fields=Overview,Genres,DateCreated,ProductionYear,ImageTags,BackdropImageTags",
        series_id, user_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch seasons for {}: status {}",
            series_id,
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse seasons response: {}", e))
    })?;

    Ok(data)
}

/// Fetch episodes for a season from the Jellyfin server.
/// Used as fallback when episodes aren't in local DB (pre-sync).
pub async fn fetch_episodes(
    client: &JellyfinClient,
    user_id: &str,
    series_id: &str,
    season_id: &str,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Shows/{}/Episodes?SeasonId={}&UserId={}&Fields=Overview,Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags",
        series_id, season_id, user_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch episodes for season {}: status {}",
            season_id,
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse episodes response: {}", e))
    })?;

    Ok(data)
}

/// Jellyfin person object returned from the People field or /Items/{id} endpoint.
#[derive(Debug, Deserialize)]
pub struct JellyfinPerson {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name", default)]
    pub name: Option<String>,
    #[serde(alias = "Role")]
    pub role: Option<String>,
    #[serde(alias = "Type")]
    pub person_type: Option<String>,
    #[serde(alias = "PrimaryImageTag")]
    pub primary_image_tag: Option<String>,
}

/// Jellyfin item response that includes People field.
#[derive(Debug, Deserialize)]
pub struct JellyfinItemWithPeople {
    #[serde(alias = "People", default)]
    pub people: Vec<JellyfinPerson>,
}

/// Fetch the cast & crew (people) for a specific item from the Jellyfin server.
pub async fn fetch_item_people(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<Vec<JellyfinPerson>, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/{}?Fields=People",
        user_id, item_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch people for {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let data: JellyfinItemWithPeople = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse people response: {}", e))
    })?;

    Ok(data.people)
}

/// Fetch similar/related items for a specific item from the Jellyfin server.
pub async fn fetch_similar_items(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Items/{}/Similar?UserId={}&Limit={}&Fields=Overview,Genres,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags",
        item_id, user_id, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch similar items for {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse similar items response: {}", e))
    })?;

    Ok(data)
}

// ── User data mutations (mark played, favorite) ─────────────────────────

/// Mark an item as played for a user.
pub async fn mark_played(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<(), JfgoatError> {
    let path = format!("/Users/{}/PlayedItems/{}", user_id, item_id);
    let resp = client.post_empty(&path).await?;
    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to mark item {} as played: status {}",
            item_id, resp.status()
        )));
    }
    Ok(())
}

/// Mark an item as unplayed for a user.
pub async fn mark_unplayed(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<(), JfgoatError> {
    let path = format!("/Users/{}/PlayedItems/{}", user_id, item_id);
    let resp = client.delete(&path).await?;
    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to mark item {} as unplayed: status {}",
            item_id, resp.status()
        )));
    }
    Ok(())
}

async fn report_playback_state(
    client: &JellyfinClient,
    path: &str,
    item_id: &str,
    position_ticks: i64,
) -> Result<(), JfgoatError> {
    let body = serde_json::json!({
        "ItemId": item_id,
        "PositionTicks": position_ticks,
    });

    let resp = client.post_json(path, &body).await?;
    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to report playback state '{}' for {}: status {}",
            path,
            item_id,
            resp.status()
        )));
    }

    Ok(())
}

/// Report playback start to Jellyfin session API.
pub async fn report_playback_playing(
    client: &JellyfinClient,
    item_id: &str,
    position_ticks: i64,
) -> Result<(), JfgoatError> {
    report_playback_state(client, "/Sessions/Playing", item_id, position_ticks).await
}

/// Report playback progress heartbeat to Jellyfin session API.
pub async fn report_playback_progress(
    client: &JellyfinClient,
    item_id: &str,
    position_ticks: i64,
) -> Result<(), JfgoatError> {
    report_playback_state(client, "/Sessions/Playing/Progress", item_id, position_ticks).await
}

/// Report playback stop to Jellyfin session API.
pub async fn report_playback_stopped(
    client: &JellyfinClient,
    item_id: &str,
    position_ticks: i64,
) -> Result<(), JfgoatError> {
    report_playback_state(client, "/Sessions/Playing/Stopped", item_id, position_ticks).await
}

/// Mark an item as favorite for a user.
pub async fn mark_favorite(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<(), JfgoatError> {
    let path = format!("/Users/{}/FavoriteItems/{}", user_id, item_id);
    let resp = client.post_empty(&path).await?;
    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to mark item {} as favorite: status {}",
            item_id, resp.status()
        )));
    }
    Ok(())
}

/// Remove an item from favorites for a user.
pub async fn unmark_favorite(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<(), JfgoatError> {
    let path = format!("/Users/{}/FavoriteItems/{}", user_id, item_id);
    let resp = client.delete(&path).await?;
    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to remove item {} from favorites: status {}",
            item_id, resp.status()
        )));
    }
    Ok(())
}

/// Search items directly on the remote Jellyfin server (fallback during INITIAL_SYNC).
pub async fn search_remote(
    client: &JellyfinClient,
    user_id: &str,
    query: &str,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?searchTerm={}&Limit={}&Recursive=true&Fields=Overview,Genres,ProductionYear,ImageTags",
        user_id,
        urlencoding::encode(query),
        limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Remote search failed: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse search response: {}", e))
    })?;

    Ok(data)
}

// ── MediaStreams and ExternalUrls ────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct JellyfinMediaStream {
    #[serde(alias = "Codec")]
    pub codec: Option<String>,
    #[serde(alias = "Language")]
    pub language: Option<String>,
    #[serde(alias = "DisplayTitle")]
    pub display_title: Option<String>,
    #[serde(alias = "Type")]
    pub stream_type: Option<String>,
    #[serde(alias = "IsDefault")]
    pub is_default: Option<bool>,
    #[serde(alias = "DeliveryMethod")]
    pub delivery_method: Option<String>,
    #[serde(alias = "IsExternal")]
    pub is_external: Option<bool>,
    #[serde(alias = "Index")]
    pub index: Option<i64>,
    #[serde(alias = "Height")]
    pub height: Option<i64>,
    #[serde(alias = "Width")]
    pub width: Option<i64>,
    #[serde(alias = "BitRate")]
    pub bit_rate: Option<i64>,
    #[serde(alias = "Channels")]
    pub channels: Option<i64>,
    #[serde(alias = "ChannelLayout")]
    pub channel_layout: Option<String>,
    #[serde(alias = "VideoRange")]
    pub video_range: Option<String>,
    #[serde(alias = "VideoRangeType")]
    pub video_range_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinExternalUrl {
    #[serde(alias = "Name")]
    pub name: Option<String>,
    #[serde(alias = "Url")]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinItemWithStreams {
    #[serde(alias = "MediaStreams", default)]
    pub media_streams: Vec<JellyfinMediaStream>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinItemWithExternals {
    #[serde(alias = "ExternalUrls", default)]
    pub external_urls: Vec<JellyfinExternalUrl>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinChapter {
    #[serde(alias = "Name")]
    pub name: Option<String>,
    #[serde(alias = "StartPositionTicks")]
    pub start_position_ticks: Option<i64>,
    #[serde(alias = "ImageTag")]
    pub image_tag: Option<String>,
    #[serde(alias = "MarkerType")]
    pub marker_type: Option<String>,
    #[serde(alias = "ChapterType")]
    pub chapter_type: Option<String>,
}

/// Fetch media streams (video, audio, subtitle tracks) for a specific item.
pub async fn fetch_item_media_streams(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<Vec<JellyfinMediaStream>, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/{}?Fields=MediaStreams",
        user_id, item_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch media streams for {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let data: JellyfinItemWithStreams = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse media streams response: {}", e))
    })?;

    Ok(data.media_streams)
}

/// Fetch external URLs (IMDb, TMDB, etc.) for a specific item.
pub async fn fetch_item_external_urls(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<Vec<JellyfinExternalUrl>, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/{}?Fields=ExternalUrls",
        user_id, item_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch external URLs for {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let data: JellyfinItemWithExternals = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse external URLs response: {}", e))
    })?;

    Ok(data.external_urls)
}

/// Fetch chapter markers for a specific item.
pub async fn fetch_item_chapters(
    client: &JellyfinClient,
    item_id: &str,
) -> Result<Vec<JellyfinChapter>, JfgoatError> {
    let path = format!("/Items/{}/Chapters", item_id);

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch chapters for {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let data: Vec<JellyfinChapter> = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse chapters response: {}", e))
    })?;

    Ok(data)
}
