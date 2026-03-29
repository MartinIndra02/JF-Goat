import type { HomepageCache, MediaItem } from "./types";

const SUPPRESSED_NEXT_UP_KEY = "jfgoat.homepage.suppressedNextUp.v1";
const DEFAULT_SUPPRESSION_MS = 15 * 60 * 1000;

export const HOMEPAGE_CACHE_UPDATED_EVENT = "jfgoat:homepage-cache-updated";

type SuppressedMap = Record<string, number>;

function canUseStorage(): boolean {
  return typeof localStorage !== "undefined";
}

function readSuppressedMap(): SuppressedMap {
  if (!canUseStorage()) return {};

  try {
    const raw = localStorage.getItem(SUPPRESSED_NEXT_UP_KEY);
    if (!raw) return {};

    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") return {};

    const now = Date.now();
    const cleaned: SuppressedMap = {};
    let hasExpiredEntries = false;
    for (const [id, expiresAt] of Object.entries(
      parsed as Record<string, unknown>,
    )) {
      if (typeof expiresAt !== "number") continue;
      if (!Number.isFinite(expiresAt)) continue;
      if (expiresAt <= now) {
        hasExpiredEntries = true;
        continue;
      }
      cleaned[id] = expiresAt;
    }

    if (hasExpiredEntries) {
      writeSuppressedMap(cleaned);
    }

    return cleaned;
  } catch {
    return {};
  }
}

function writeSuppressedMap(map: SuppressedMap): void {
  if (!canUseStorage()) return;

  try {
    localStorage.setItem(SUPPRESSED_NEXT_UP_KEY, JSON.stringify(map));
  } catch {
    // Best effort only.
  }
}

export function suppressNextUpItem(
  itemId: string,
  ttlMs = DEFAULT_SUPPRESSION_MS,
): void {
  if (!itemId) return;

  const map = readSuppressedMap();
  map[itemId] = Date.now() + Math.max(1_000, ttlMs);
  writeSuppressedMap(map);
}

export function filterSuppressedNextUp(items: MediaItem[]): MediaItem[] {
  if (!items.length) return items;

  const map = readSuppressedMap();
  if (Object.keys(map).length === 0) return items;

  return items.filter((item) => {
    if (!item?.id) return true;
    return !map[item.id];
  });
}

function safeMediaItems(items: MediaItem[] | null | undefined): MediaItem[] {
  if (!Array.isArray(items)) return [];
  return items.filter((item) => typeof item?.id === "string" && item.id.length > 0);
}

export function applyEpisodeCompletionToHomepageCache(
  cache: HomepageCache,
  completedEpisodeId: string,
  nextEpisode: MediaItem | null,
): HomepageCache {
  const resumeItems = safeMediaItems(cache.resume_items).filter(
    (item) => item.id !== completedEpisodeId,
  );
  const nextUpItems = safeMediaItems(cache.next_up_items).filter(
    (item) => item.id !== completedEpisodeId,
  );

  if (!nextEpisode?.id || nextEpisode.id === completedEpisodeId) {
    return {
      ...cache,
      resume_items: resumeItems,
      next_up_items: nextUpItems,
    };
  }

  const alreadyListed = nextUpItems.some((item) => item.id === nextEpisode.id);
  const mergedNextUp = alreadyListed
    ? nextUpItems
    : [nextEpisode, ...nextUpItems];

  return {
    ...cache,
    resume_items: resumeItems,
    next_up_items: mergedNextUp,
  };
}

export function emitHomepageCacheUpdated(cache: HomepageCache): void {
  if (typeof window === "undefined") return;

  window.dispatchEvent(
    new CustomEvent<HomepageCache>(HOMEPAGE_CACHE_UPDATED_EVENT, {
      detail: cache,
    }),
  );
}
