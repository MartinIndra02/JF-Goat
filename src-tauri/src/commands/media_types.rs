use serde::{Deserialize, Serialize};
use crate::db::media::MediaItem;

/// A person (actor, director, etc.) associated with a media item.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    pub person_type: Option<String>,
    pub image_tag: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserLibrary {
    pub id: String,
    pub name: String,
    pub collection_type: Option<String>,
}

// ── Homepage cache for instant startup ──────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HomepageCache {
    pub resume_items: Vec<MediaItem>,
    pub next_up_items: Vec<MediaItem>,
    pub user_libraries: Vec<UserLibrary>,
    pub library_latest: std::collections::HashMap<String, Vec<MediaItem>>,
    pub featured_items: Vec<MediaItem>,
    #[serde(default)]
    pub cache_refreshed_at_epoch_ms: Option<u64>,
}

/// A single stream option (video, audio, or subtitle track).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamOption {
    pub index: i64,
    pub codec: String,
    pub display_title: String,
    pub language: Option<String>,
    pub is_default: bool,
    pub delivery_method: Option<String>,
    pub is_external: bool,
    pub height: Option<i64>,
    pub width: Option<i64>,
    pub bit_rate: Option<i64>,
    pub channels: Option<i64>,
    pub channel_layout: Option<String>,
    pub video_range: Option<String>,
    pub video_range_type: Option<String>,
}

/// Grouped media stream info for an item.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaStreamInfo {
    pub video: Vec<StreamOption>,
    pub audio: Vec<StreamOption>,
    pub subtitle: Vec<StreamOption>,
    /// Short label for the primary video stream, e.g. "HD SDR"
    pub video_label: Option<String>,
    pub original_size: Option<i64>,
}

/// An external URL (e.g. IMDb, TMDB, TheTVDB).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExternalUrl {
    pub name: String,
    pub url: String,
}

/// A single chapter marker in a media item timeline.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChapterInfo {
    pub name: String,
    pub start_ticks: i64,
    pub image_tag: Option<String>,
    pub marker_type: Option<String>,
    pub chapter_type: Option<String>,
}

/// A media segment (intro, outro, recap, etc.) with start/end in ticks.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaSegment {
    pub start_ticks: i64,
    pub end_ticks: i64,
    pub segment_type: String, // "Intro", "Outro", "Recap", "Commercial", "Preview"
}

