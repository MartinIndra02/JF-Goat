import { invoke } from "@tauri-apps/api/core";
import type {
  ServerPublicInfo,
  LoginResult,
  SessionInfo,
  SearchResult,
  MediaItem,
  UserLibrary,
  HomepageCache,
  Person,
  MediaStreamInfo,
  ExternalUrl,
} from "./types";

export async function connectToServer(url: string): Promise<ServerPublicInfo> {
  return invoke("connect_to_server", { url });
}

export async function login(
  username: string,
  password: string,
): Promise<LoginResult> {
  return invoke("login", { username, password });
}

export async function checkAuth(): Promise<SessionInfo | null> {
  return invoke("check_auth");
}

export async function checkAuthOffline(): Promise<SessionInfo | null> {
  return invoke("check_auth_offline");
}

export async function logout(): Promise<void> {
  return invoke("logout");
}

export async function startSync(): Promise<void> {
  return invoke("start_sync");
}

export async function searchItems(query: string): Promise<SearchResult> {
  return invoke("search_items", { query });
}

export async function getSyncStatus(): Promise<string> {
  return invoke("get_sync_status");
}

export async function forceResync(): Promise<void> {
  return invoke("force_resync");
}

export async function getRecentMovies(limit: number): Promise<MediaItem[]> {
  return invoke("get_recent_movies", { limit });
}

export async function getRecentSeries(limit: number): Promise<MediaItem[]> {
  return invoke("get_recent_series", { limit });
}

export async function getContinueWatching(limit: number): Promise<MediaItem[]> {
  return invoke("get_continue_watching", { limit });
}

export async function getLatestMedia(limit: number): Promise<MediaItem[]> {
  return invoke("get_latest_media", { limit });
}

// ── Live Jellyfin API commands ──────────────────────────────────────────

export async function getUserViews(): Promise<UserLibrary[]> {
  return invoke("get_user_views");
}

export async function getResumeItems(limit: number): Promise<MediaItem[]> {
  return invoke("get_resume_items", { limit });
}

export async function getNextUp(limit: number): Promise<MediaItem[]> {
  return invoke("get_next_up", { limit });
}

export async function getLatestItems(
  parentId: string,
  limit: number,
): Promise<MediaItem[]> {
  return invoke("get_latest_items", { parentId, limit });
}

// ── Detail page commands ─────────────────────────────────────────────

export async function getItemById(id: string): Promise<MediaItem | null> {
  return invoke("get_item_by_id", { id });
}

export async function getSeriesSeasons(seriesId: string): Promise<MediaItem[]> {
  return invoke("get_series_seasons", { seriesId });
}

export async function getSeasonEpisodes(
  seasonId: string,
): Promise<MediaItem[]> {
  return invoke("get_season_episodes", { seasonId });
}

export async function getItemPeople(id: string): Promise<Person[]> {
  return invoke("get_item_people", { id });
}

export async function getSimilarItems(
  id: string,
  limit: number,
): Promise<MediaItem[]> {
  return invoke("get_similar_items", { id, limit });
}

// ── Homepage cache commands ─────────────────────────────────────────────

export async function saveHomepageCache(data: HomepageCache): Promise<void> {
  return invoke("save_homepage_cache", { data });
}

export async function loadHomepageCache(): Promise<HomepageCache | null> {
  return invoke("load_homepage_cache");
}

// ── MPV player commands ─────────────────────────────────────────

export async function mpvPlay(
  itemId: string,
  startTicks: number,
): Promise<void> {
  return invoke("mpv_play", { itemId, startTicks });
}

export async function mpvTogglePause(): Promise<void> {
  return invoke("mpv_toggle_pause");
}

export async function mpvSeek(seconds: number): Promise<void> {
  return invoke("mpv_seek", { seconds });
}

export async function mpvSeekAbsolute(seconds: number): Promise<void> {
  return invoke("mpv_seek_absolute", { seconds });
}

export async function mpvSetVolume(volume: number): Promise<void> {
  return invoke("mpv_set_volume", { volume });
}

export async function mpvStop(): Promise<void> {
  return invoke("mpv_stop");
}

export async function getMediaStreams(id: string): Promise<MediaStreamInfo> {
  return invoke("get_media_streams", { id });
}

export async function getExternalUrls(id: string): Promise<ExternalUrl[]> {
  return invoke("get_external_urls", { id });
}

// ── User data mutations ──────────────────────────────────────────────

export async function togglePlayed(
  id: string,
  played: boolean,
): Promise<boolean> {
  return invoke("toggle_played", { id, played });
}

export async function toggleFavorite(
  id: string,
  isFavorite: boolean,
): Promise<boolean> {
  return invoke("toggle_favorite", { id, isFavorite });
}
