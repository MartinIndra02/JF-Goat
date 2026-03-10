import type { SessionInfo } from "../types";

export type AuthStatus = "loading" | "unauthenticated" | "authenticated";

let status = $state<AuthStatus>("loading");
let session = $state<SessionInfo | null>(null);

export function getAuthStatus(): AuthStatus {
  return status;
}

export function getSession(): SessionInfo | null {
  return session;
}

export function setAuthenticated(info: SessionInfo) {
  session = info;
  status = "authenticated";
}

export function setUnauthenticated() {
  session = null;
  status = "unauthenticated";
}

export function setLoading() {
  status = "loading";
}
