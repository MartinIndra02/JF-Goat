import { listen } from "@tauri-apps/api/event";
import type { MpvTimeUpdate, MpvStateChange } from "../types";

export type PlayerStatus = "idle" | "loading" | "playing" | "paused" | "ended";

// ── Reactive state ──────────────────────────────────────────────

let status = $state<PlayerStatus>("idle");
let visible = $state(false);
let title = $state("");
let itemId = $state<string | null>(null);
let timePos = $state(0);
let duration = $state(0);
let volume = $state(100);

// ── Getters ─────────────────────────────────────────────────────

export function getPlayerStatus(): PlayerStatus {
  return status;
}

export function isPlayerVisible(): boolean {
  return visible;
}

export function getPlayerTitle(): string {
  return title;
}

export function getPlayerItemId(): string | null {
  return itemId;
}

export function getTimePos(): number {
  return timePos;
}

export function getDuration(): number {
  return duration;
}

export function getVolume(): number {
  return volume;
}

// ── Actions ─────────────────────────────────────────────────────

export function showPlayer(id: string, displayTitle: string) {
  itemId = id;
  title = displayTitle;
  status = "loading";
  visible = true;
  timePos = 0;
  duration = 0;
}

export function hidePlayer() {
  visible = false;
  status = "idle";
  itemId = null;
  title = "";
  timePos = 0;
  duration = 0;
}

export function setVolume(v: number) {
  volume = Math.max(0, Math.min(100, v));
}

// ── Event listeners (called once from App.svelte) ───────────────

let listenersAttached = false;

export function initPlayerListeners() {
  if (listenersAttached) return;
  listenersAttached = true;

  listen<MpvTimeUpdate>("mpv-time-update", (event) => {
    timePos = event.payload.position;
    duration = event.payload.duration;
    if (status === "loading") {
      status = "playing";
    }
  });

  listen<MpvStateChange>("mpv-state-change", (event) => {
    status = event.payload.paused ? "paused" : "playing";
  });

  listen("mpv-file-ended", () => {
    status = "ended";
  });

  listen("mpv-stopped", () => {
    status = "idle";
    visible = false;
  });
}
