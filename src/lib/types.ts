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
