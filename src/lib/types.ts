export interface ServerPublicInfo {
  id: string;
  name: string;
  version: string;
  url: string;
}

export interface LoginResult {
  user_id: string;
  username: string;
  server_id: string;
}

export interface SessionInfo {
  user_id: string;
  username: string;
  server_id: string;
  server_name: string;
  server_url: string;
}

export interface JfgoatError {
  kind: string;
  message: string;
}

export interface MediaItem {
  id: string;
  name: string;
  type: string;
  parent_id: string | null;
  series_id: string | null;
  series_name: string | null;
  season_id: string | null;
  season_name: string | null;
  index_number: number | null;
  production_year: number | null;
  overview: string | null;
  image_tag: string | null;
  backdrop_tag: string | null;
  date_created: string | null;
  date_updated: string | null;
  community_rating: number | null;
  official_rating: string | null;
  genres: string | null;
  run_time_ticks: number | null;
  played: boolean;
  play_count: number;
  playback_ticks: number;
  is_favorite: boolean;
  server_id: string;
}

export interface UserLibrary {
  id: string;
  name: string;
  collection_type: string | null;
}

export interface SearchResult {
  items: MediaItem[];
  source: "local" | "remote";
}

export interface SyncProgress {
  current: number;
  total: number;
  percentage: number;
}

export interface SyncError {
  message: string;
  batch_start: number;
  batch_size: number;
  retries_attempted: number;
  is_fatal: boolean;
}

export interface HomepageCache {
  resume_items: MediaItem[];
  next_up_items: MediaItem[];
  user_libraries: UserLibrary[];
  library_latest: Record<string, MediaItem[]>;
  featured_items: MediaItem[];
}

// ── Player types ────────────────────────────────────────────────

export interface MpvTimeUpdate {
  position: number;
  duration: number;
}

export interface MpvStateChange {
  paused: boolean;
}

export type VideoScaleMode = "contain" | "cover" | "stretch";

export interface MpvPlaybackSettings {
  volume: number;
  muted: boolean;
  playback_rate: number;
  video_scale_mode: VideoScaleMode;
  audio_track: number | null;
  subtitle_track: number | null;
}

export interface PlaybackRequest {
  itemId: string;
  startTicks: number;
  audioStreamIndex?: number | null;
  subtitleStreamIndex?: number | null;
  maxStreamingBitrate?: number | null;
  targetHeight?: number | null;
}

export interface PlaybackSelection {
  audioStreamIndex?: number | null;
  subtitleStreamIndex?: number | null;
  audioLanguage?: string | null;
  subtitleLanguage?: string | null;
  audioDisplayTitle?: string | null;
  subtitleDisplayTitle?: string | null;
  maxStreamingBitrate?: number | null;
  targetHeight?: number | null;
}

export interface Person {
  id: string;
  name: string;
  role: string | null;
  person_type: string | null;
  image_tag: string | null;
}

export interface StreamOption {
  index: number;
  codec: string;
  display_title: string;
  language: string | null;
  is_default: boolean;
  delivery_method?: string | null;
  is_external?: boolean | null;
  height?: number | null;
  width?: number | null;
  bit_rate?: number | null;
  channels?: number | null;
  channel_layout?: string | null;
  video_range?: string | null;
  video_range_type?: string | null;
}

export interface MediaStreamInfo {
  video: StreamOption[];
  audio: StreamOption[];
  subtitle: StreamOption[];
  video_label: string | null;
}

export interface ChapterInfo {
  name: string;
  start_ticks: number;
  image_tag: string | null;
  marker_type?: string | null;
  chapter_type?: string | null;
}

export interface ExternalUrl {
  name: string;
  url: string;
}
