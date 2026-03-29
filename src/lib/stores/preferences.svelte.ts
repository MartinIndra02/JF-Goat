import { getUserPreferences, saveUserPreferences } from "../api";
import {
  DEFAULT_USER_PREFERENCES,
  type UserPreferences,
} from "../types";
import { pushErrorToast } from "./toast.svelte";

const LOCAL_PREFS_KEY = "jfgoat.user.preferences.v1";

let preferences = $state<UserPreferences>(cloneDefaults());
let loaded = $state(false);
let saving = $state(false);
let saveTimer: ReturnType<typeof setTimeout> | null = null;

type PreferencesUpdate = {
  playback?: Partial<UserPreferences["playback"]>;
  language?: Partial<UserPreferences["language"]>;
  quality?: Partial<UserPreferences["quality"]>;
  cache?: Partial<UserPreferences["cache"]>;
  refresh_interval_seconds?: UserPreferences["refresh_interval_seconds"];
};

function cloneDefaults(): UserPreferences {
  return {
    playback: { ...DEFAULT_USER_PREFERENCES.playback },
    language: { ...DEFAULT_USER_PREFERENCES.language },
    quality: { ...DEFAULT_USER_PREFERENCES.quality },
    cache: { ...DEFAULT_USER_PREFERENCES.cache },
    refresh_interval_seconds: DEFAULT_USER_PREFERENCES.refresh_interval_seconds,
  };
}

function sanitizePreferences(input: UserPreferences): UserPreferences {
  const playbackRate = Number.isFinite(input.playback.default_playback_rate)
    ? input.playback.default_playback_rate
    : DEFAULT_USER_PREFERENCES.playback.default_playback_rate;

  const refreshInterval = Number.isFinite(input.refresh_interval_seconds)
    ? input.refresh_interval_seconds
    : DEFAULT_USER_PREFERENCES.refresh_interval_seconds;

  const cacheMaxAge = Number.isFinite(input.cache.max_age_minutes)
    ? input.cache.max_age_minutes
    : DEFAULT_USER_PREFERENCES.cache.max_age_minutes;

  return {
    playback: {
      autoplay_next_episode: !!input.playback.autoplay_next_episode,
      default_playback_rate: Math.max(0.5, Math.min(2, playbackRate)),
    },
    language: {
      preferred_audio_language: (input.language.preferred_audio_language ?? "")
        .trim()
        .toLowerCase(),
      preferred_subtitle_language: (input.language.preferred_subtitle_language ?? "")
        .trim()
        .toLowerCase(),
    },
    quality: {
      default_quality_key: (input.quality.default_quality_key ?? "").trim() || "direct-play",
    },
    cache: {
      enabled: !!input.cache.enabled,
      max_age_minutes: Math.max(5, Math.min(10_080, Math.round(cacheMaxAge))),
    },
    refresh_interval_seconds: Math.max(30, Math.min(1_800, Math.round(refreshInterval))),
  };
}

function mergePreferences(
  current: UserPreferences,
  update: PreferencesUpdate,
): UserPreferences {
  return sanitizePreferences({
    playback: {
      ...current.playback,
      ...(update.playback ?? {}),
    },
    language: {
      ...current.language,
      ...(update.language ?? {}),
    },
    quality: {
      ...current.quality,
      ...(update.quality ?? {}),
    },
    cache: {
      ...current.cache,
      ...(update.cache ?? {}),
    },
    refresh_interval_seconds:
      update.refresh_interval_seconds ?? current.refresh_interval_seconds,
  });
}

function readLocalFallback(): UserPreferences | null {
  if (typeof localStorage === "undefined") return null;

  try {
    const raw = localStorage.getItem(LOCAL_PREFS_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as UserPreferences;
    return sanitizePreferences(parsed);
  } catch {
    return null;
  }
}

function writeLocalFallback(next: UserPreferences): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(LOCAL_PREFS_KEY, JSON.stringify(next));
  } catch {
    // Best effort only.
  }
}

function queuePersist(next: UserPreferences): void {
  if (saveTimer) {
    clearTimeout(saveTimer);
  }

  saveTimer = setTimeout(async () => {
    saveTimer = null;
    saving = true;
    try {
      const persisted = await saveUserPreferences(next);
      preferences = sanitizePreferences(persisted);
      writeLocalFallback(preferences);
    } catch (error) {
      pushErrorToast("api", error, "Could not save settings", "settings-save-error");
      writeLocalFallback(next);
    } finally {
      saving = false;
    }
  }, 300);
}

export function getPreferences(): UserPreferences {
  return preferences;
}

export function isPreferencesLoaded(): boolean {
  return loaded;
}

export function isPreferencesSaving(): boolean {
  return saving;
}

export async function loadPreferences(): Promise<UserPreferences> {
  if (loaded) return preferences;

  const local = readLocalFallback();
  if (local) {
    preferences = local;
  }

  try {
    const remote = await getUserPreferences();
    preferences = sanitizePreferences(remote);
    writeLocalFallback(preferences);
  } catch (error) {
    if (!local) {
      preferences = cloneDefaults();
      writeLocalFallback(preferences);
    }
    pushErrorToast(
      "api",
      error,
      "Using default settings",
      "settings-load-error",
    );
  }

  loaded = true;
  return preferences;
}

export function updatePreferences(update: PreferencesUpdate): UserPreferences {
  const next = mergePreferences(preferences, update);
  preferences = next;
  writeLocalFallback(next);
  queuePersist(next);
  return next;
}

export function replacePreferences(next: UserPreferences): UserPreferences {
  const sanitized = sanitizePreferences(next);
  preferences = sanitized;
  writeLocalFallback(sanitized);
  queuePersist(sanitized);
  return sanitized;
}
