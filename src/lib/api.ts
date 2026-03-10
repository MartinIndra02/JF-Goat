import { invoke } from "@tauri-apps/api/core";
import type { ServerPublicInfo, LoginResult, SessionInfo } from "./types";

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

export async function logout(): Promise<void> {
  return invoke("logout");
}
