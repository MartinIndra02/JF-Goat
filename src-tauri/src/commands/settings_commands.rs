use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::Manager;

use crate::error::JfgoatError;
use crate::state::AppState;
use crate::download::{get_download_dir, build_ctx};

const USER_PREFERENCES_KEY: &str = "user_preferences_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackPreferences {
    pub autoplay_next_episode: bool,
    pub default_playback_rate: f64,
    pub hwdec: String,
    pub skip_forward_seconds: f64,
    pub skip_backward_seconds: f64,
    pub subtitle_size_percent: i64,
    pub subtitle_color: String,
    pub subtitle_background_opacity: i64,
    pub default_startup_screen: String,
    pub auto_crop_experimental: bool,
    pub auto_crop_mode: String,
}

impl Default for PlaybackPreferences {
    fn default() -> Self {
        Self {
            autoplay_next_episode: true,
            default_playback_rate: 1.0,
            hwdec: "auto".to_string(),
            skip_forward_seconds: 30.0,
            skip_backward_seconds: 10.0,
            subtitle_size_percent: 100,
            subtitle_color: "#ffffff".to_string(),
            subtitle_background_opacity: 0,
            default_startup_screen: "/home".to_string(),
            auto_crop_experimental: false,
            auto_crop_mode: "static".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguagePreferences {
    pub preferred_audio_language: String,
    pub preferred_subtitle_language: String,
}

impl Default for LanguagePreferences {
    fn default() -> Self {
        Self {
            preferred_audio_language: String::new(),
            preferred_subtitle_language: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct QualityPreferences {
    pub default_quality_key: String,
}

impl Default for QualityPreferences {
    fn default() -> Self {
        Self {
            default_quality_key: "direct-play".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CachePreferences {
    pub enabled: bool,
    pub max_age_minutes: u32,
}

impl Default for CachePreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            max_age_minutes: 720,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPreferences {
    pub playback: PlaybackPreferences,
    pub language: LanguagePreferences,
    pub quality: QualityPreferences,
    pub cache: CachePreferences,
    pub refresh_interval_seconds: u32,
    pub download_directory: Option<String>,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            playback: PlaybackPreferences::default(),
            language: LanguagePreferences::default(),
            quality: QualityPreferences::default(),
            cache: CachePreferences::default(),
            refresh_interval_seconds: 180,
            download_directory: None,
        }
    }
}

impl UserPreferences {
    fn sanitize(mut self) -> Self {
        self.playback.default_playback_rate =
            self.playback.default_playback_rate.clamp(0.5, 2.0);

        self.playback.skip_forward_seconds = self.playback.skip_forward_seconds.clamp(5.0, 300.0);
        self.playback.skip_backward_seconds = self.playback.skip_backward_seconds.clamp(5.0, 300.0);
        self.playback.subtitle_size_percent = self.playback.subtitle_size_percent.clamp(50, 300);
        self.playback.subtitle_background_opacity = self.playback.subtitle_background_opacity.clamp(0, 100);
        if self.playback.hwdec.is_empty() {
            self.playback.hwdec = "auto".to_string();
        }
        if self.playback.subtitle_color.is_empty() {
            self.playback.subtitle_color = "#ffffff".to_string();
        }
        if self.playback.default_startup_screen.is_empty() {
            self.playback.default_startup_screen = "/home".to_string();
        }
        if self.playback.auto_crop_mode != "static" && self.playback.auto_crop_mode != "dynamic" {
            self.playback.auto_crop_mode = "static".to_string();
        }

        self.language.preferred_audio_language = self
            .language
            .preferred_audio_language
            .trim()
            .to_lowercase();
        self.language.preferred_subtitle_language = self
            .language
            .preferred_subtitle_language
            .trim()
            .to_lowercase();

        self.quality.default_quality_key = self.quality.default_quality_key.trim().to_string();
        if self.quality.default_quality_key.is_empty() {
            self.quality.default_quality_key = "direct-play".to_string();
        }

        self.cache.max_age_minutes = self.cache.max_age_minutes.clamp(5, 10_080);
        self.refresh_interval_seconds = self.refresh_interval_seconds.clamp(30, 1_800);

        self.download_directory = self.download_directory.map(|d| d.trim().to_string());

        self
    }
}

#[tauri::command]
pub fn get_user_preferences(
    state: State<'_, AppState>,
) -> Result<UserPreferences, JfgoatError> {
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let maybe_raw = db.query_row(
        "SELECT value FROM metadata WHERE key = ?1",
        rusqlite::params![USER_PREFERENCES_KEY],
        |row| row.get::<_, String>(0),
    );

    match maybe_raw {
        Ok(raw) => {
            let parsed = serde_json::from_str::<UserPreferences>(&raw)
                .unwrap_or_else(|_| UserPreferences::default());
            Ok(parsed.sanitize())
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(UserPreferences::default()),
        Err(e) => Err(e.into()),
    }
}

#[tauri::command]
pub fn save_user_preferences(
    state: State<'_, AppState>,
    preferences: UserPreferences,
) -> Result<UserPreferences, JfgoatError> {
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sanitized = preferences.sanitize();
    let json = serde_json::to_string(&sanitized)
        .map_err(|e| JfgoatError::Internal(format!("JSON serialize error: {}", e)))?;

    db.execute(
        "INSERT INTO metadata (key, value)
         VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![USER_PREFERENCES_KEY, json],
    )?;

    Ok(sanitized)
}

#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    app.restart();
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageUsage {
    pub cache_bytes: u64,
    pub downloads_bytes: u64,
}

#[tauri::command]
pub fn get_storage_usage(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<StorageUsage, JfgoatError> {
    let cache_dir = app_handle.path().app_cache_dir().unwrap_or_default();
    let cache_bytes = get_dir_size(&cache_dir).unwrap_or(0);

    let ctx = build_ctx(&state);
    let download_dir = get_download_dir(app_handle.clone(), &ctx)?;
    let downloads_bytes = get_dir_size(&download_dir).unwrap_or(0);

    Ok(StorageUsage {
        cache_bytes,
        downloads_bytes,
    })
}

#[tauri::command]
pub fn clear_app_cache(
    app_handle: tauri::AppHandle,
) -> Result<(), JfgoatError> {
    let cache_dir = app_handle.path().app_cache_dir().unwrap_or_default();
    if cache_dir.exists() {
        let _ = std::fs::remove_dir_all(&cache_dir);
        let _ = std::fs::create_dir_all(&cache_dir);
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_all_offline_downloads(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), JfgoatError> {
    let downloads = {
        let db = state.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        let mut stmt = db.prepare("SELECT id FROM offline_downloads")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut ids = Vec::new();
        for r in rows {
            if let Ok(id) = r {
                ids.push(id);
            }
        }
        ids
    };

    for id in downloads {
        let _ = super::download_commands::delete_download(state.clone(), app_handle.clone(), id).await;
    }
    Ok(())
}

fn get_dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut size = 0;
    if path.is_file() {
        return Ok(std::fs::metadata(path)?.len());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            size += get_dir_size(&p)?;
        } else {
            size += entry.metadata()?.len();
        }
    }
    Ok(size)
}
