import { invoke } from "@tauri-apps/api/core";
import type {
  ServerPublicInfo,
  LoginResult,
  SessionInfo,
  SearchResult,
  MediaItem,
  UserLibrary,
  HomepageCache,
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

// ── Homepage cache commands ─────────────────────────────────────────────

export async function saveHomepageCache(
  data: HomepageCache,
): Promise<void> {
  return invoke("save_homepage_cache", { data });
}

export async function loadHomepageCache(): Promise<HomepageCache | null> {
  return invoke("load_homepage_cache");
}
