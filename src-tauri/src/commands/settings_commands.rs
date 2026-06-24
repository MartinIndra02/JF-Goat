use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::JfgoatError;
use crate::state::AppState;

const USER_PREFERENCES_KEY: &str = "user_preferences_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackPreferences {
    pub autoplay_next_episode: bool,
    pub default_playback_rate: f64,
}

impl Default for PlaybackPreferences {
    fn default() -> Self {
        Self {
            autoplay_next_episode: true,
            default_playback_rate: 1.0,
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
