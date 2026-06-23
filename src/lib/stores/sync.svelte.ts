import { listen } from "@tauri-apps/api/event";
import type { SyncProgress, SyncError } from "../types";

export type SyncState =
  | "idle"
  | "syncing"
  | "complete"
  | "complete_with_errors";

let syncState = $state<SyncState>("idle");
let progress = $state<SyncProgress | null>(null);
let syncErrors = $state<SyncError[]>([]);
let fatalError = $state<string | null>(null);

let listenersAttached = false;
let syncTriggered = false;

export function isSyncTriggered(): boolean {
  return syncTriggered;
}

export function markSyncTriggered() {
  syncTriggered = true;
}

export function getSyncState(): SyncState {
  return syncState;
}

export function getSyncProgress(): SyncProgress | null {
  return progress;
}

export function getSyncErrors(): SyncError[] {
  return syncErrors;
}

export function getFatalError(): string | null {
  return fatalError;
}

export function resetSyncStore() {
  syncState = "idle";
  progress = null;
  syncErrors = [];
  fatalError = null;
  listenersAttached = false;
  syncTriggered = false;
}

export function initSyncListeners() {
  if (listenersAttached) return;
  listenersAttached = true;

  listen<SyncProgress>("sync-progress", (event) => {
    syncState = "syncing";
    progress = event.payload;
  });

  listen("sync-complete", () => {
    // Only set to "complete" if not already in error state
    if (syncState !== "complete_with_errors") {
      syncState = "complete";
      setTimeout(() => {
        syncState = "idle";
        progress = null;
        syncErrors = [];
      }, 3000);
    }
  });

  listen<SyncError>("sync-error", (event) => {
    const error = event.payload;
    syncErrors = [...syncErrors, error];
    if (error.is_fatal) {
      fatalError = error.message;
    }
  });

  listen<SyncError>("sync-complete-with-errors", (event) => {
    syncState = "complete_with_errors";
    fatalError = event.payload.message;
    setTimeout(() => {
      syncState = "idle";
      progress = null;
      syncErrors = [];
      fatalError = null;
    }, 8000);
  });
}
