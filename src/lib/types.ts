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
