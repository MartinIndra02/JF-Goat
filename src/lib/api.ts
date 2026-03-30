import { invoke } from "@tauri-apps/api/core";
import type {
  ServerPublicInfo,
  LoginResult,
  SessionInfo,
  SearchResult,
  MediaItem,
  PaginatedResult,
  PaginationRequest,
  UserLibrary,
  HomepageCache,
  UserPreferences,
  Person,
  MediaStreamInfo,
  ChapterInfo,
  ExternalUrl,
  PlaybackRequest,
  PlaybackConfigPayload,
  DirectPlaybackQuery,
  TranscodePlaybackQuery,
  VideoScaleMode,
} from "./types";

function encodeQueryValue(value: string | number): string {
  return encodeURIComponent(String(value));
}

function toQueryString<T extends object>(query: T): string {
  return Object.entries(query as Record<string, string | number | undefined>)
    .filter(([, value]) => value !== undefined)
    .map(([key, value]) => `${encodeURIComponent(key)}=${encodeQueryValue(value as string | number)}`)
    .join("&");
}

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

export async function getLibraryItems(
  parentId: string,
  limit: number,
): Promise<MediaItem[]> {
  return invoke("get_latest_items", { parentId, limit });
}

export async function getLibraryItemsPage(
  parentId: string,
  pagination: PaginationRequest,
): Promise<PaginatedResult<MediaItem>> {
  // Stable DTO stub for feature work until backend paging command is introduced.
  const safeLimit = Math.max(1, Math.trunc(pagination.limit));
  const safeStartIndex = Math.max(0, Math.trunc(pagination.start_index));
  const items = await getLibraryItems(parentId, safeLimit);

  return {
    items,
    total_record_count: safeStartIndex + items.length,
    start_index: safeStartIndex,
    limit: safeLimit,
    has_more: items.length >= safeLimit,
  };
}

export function buildPlaybackConfigPayload(
  serverUrl: string,
  apiKey: string,
  request: PlaybackRequest,
): PlaybackConfigPayload {
  const normalizedServerUrl = serverUrl.replace(/\/+$/, "");
  const endpoint = `/Videos/${request.itemId}/stream`;
  const shouldTranscode =
    (request.maxStreamingBitrate ?? 0) > 0 || (request.targetHeight ?? 0) > 0;

  if (shouldTranscode) {
    const query: TranscodePlaybackQuery = {
      api_key: apiKey,
      static: "false",
    };

    if (typeof request.audioStreamIndex === "number" && request.audioStreamIndex >= 0) {
      query.AudioStreamIndex = request.audioStreamIndex;
    }

    if (typeof request.subtitleStreamIndex === "number") {
      query.SubtitleStreamIndex =
        request.subtitleStreamIndex >= 0 ? request.subtitleStreamIndex : -1;
    }

    if ((request.maxStreamingBitrate ?? 0) > 0) {
      query.MaxStreamingBitrate = request.maxStreamingBitrate as number;
    }

    if ((request.targetHeight ?? 0) > 0) {
      query.MaxHeight = request.targetHeight as number;
    }

    const url = `${normalizedServerUrl}${endpoint}?${toQueryString(query)}`;

    return {
      mode: "transcode",
      item_id: request.itemId,
      endpoint,
      url,
      query,
    };
  }

  const query: DirectPlaybackQuery = {
    api_key: apiKey,
    static: "true",
    mediaSourceId: request.itemId,
  };

  const url = `${normalizedServerUrl}${endpoint}?${toQueryString(query)}`;

  return {
    mode: "direct",
    item_id: request.itemId,
    endpoint,
    url,
    query,
  };
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

export async function getUserPreferences(): Promise<UserPreferences> {
  return invoke("get_user_preferences");
}

export async function saveUserPreferences(
  preferences: UserPreferences,
): Promise<UserPreferences> {
  return invoke("save_user_preferences", { preferences });
}

// ── MPV player commands ─────────────────────────────────────────

export async function mpvPlay(request: PlaybackRequest): Promise<void> {
  return invoke("mpv_play", { ...request });
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

export async function mpvSetMute(muted: boolean): Promise<void> {
  return invoke("mpv_set_mute", { muted });
}

export async function mpvSetPlaybackRate(rate: number): Promise<void> {
  return invoke("mpv_set_playback_rate", { rate });
}

export async function mpvSetSubtitlePosition(position: number): Promise<void> {
  return invoke("mpv_set_subtitle_position", { position });
}

export async function mpvSetVideoScale(mode: VideoScaleMode): Promise<void> {
  return invoke("mpv_set_video_scale", { mode });
}

export async function mpvSetAudioTrack(track: number): Promise<void> {
  return invoke("mpv_set_audio_track", { track });
}

export async function mpvSetSubtitleTrack(track: number | null): Promise<void> {
  return invoke("mpv_set_subtitle_track", { track });
}

export async function mpvStop(): Promise<void> {
  return invoke("mpv_stop");
}

export async function getMediaStreams(id: string): Promise<MediaStreamInfo> {
  return invoke("get_media_streams", { id });
}

export async function getItemChapters(id: string): Promise<ChapterInfo[]> {
  return invoke("get_item_chapters", { id });
}

export async function getExternalUrls(id: string): Promise<ExternalUrl[]> {
  return invoke("get_external_urls", { id });
}

export type PlaybackLifecycleEvent = "playing" | "progress" | "stopped";

export async function reportPlaybackLifecycle(
  itemId: string,
  positionTicks: number,
  durationTicks: number,
  event: PlaybackLifecycleEvent,
): Promise<void> {
  return invoke("report_playback_lifecycle", {
    itemId,
    positionTicks,
    durationTicks,
    event,
  });
}

export async function reportPlaybackStopped(
  itemId: string,
  positionTicks: number,
  durationTicks: number,
): Promise<void> {
  return reportPlaybackLifecycle(itemId, positionTicks, durationTicks, "stopped");
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
