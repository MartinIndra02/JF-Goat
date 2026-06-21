import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  applyEpisodeCompletionToHomepageCache,
  filterSuppressedNextUp,
  suppressNextUpItem,
} from "./homepageFreshness";
import type { HomepageCache, MediaItem, UserLibrary } from "./types";

function mediaItem(overrides: Partial<MediaItem>): MediaItem {
  return {
    id: "item",
    name: "Item",
    type: "Episode",
    parent_id: null,
    series_id: null,
    series_name: null,
    season_id: null,
    season_name: null,
    index_number: null,
    production_year: null,
    overview: null,
    image_tag: null,
    backdrop_tag: null,
    date_created: null,
    date_updated: null,
    community_rating: null,
    official_rating: null,
    genres: null,
    run_time_ticks: null,
    played: false,
    play_count: 0,
    playback_ticks: 0,
    is_favorite: false,
    server_id: "srv-1",
    user_id: "user-1",
    ...overrides,
  };
}

function homepageCache(overrides?: Partial<HomepageCache>): HomepageCache {
  const libs: UserLibrary[] = [
    {
      id: "lib-1",
      name: "Shows",
      collection_type: "tvshows",
    },
  ];

  return {
    resume_items: [mediaItem({ id: "ep-1", name: "Pilot" })],
    next_up_items: [mediaItem({ id: "ep-2", name: "Nexus" })],
    user_libraries: libs,
    library_latest: {},
    featured_items: [],
    ...overrides,
  };
}

describe("homepageFreshness", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("suppresses and filters next-up items until TTL expires", () => {
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(1_000);
    suppressNextUpItem("ep-1", 2_000);

    const filtered = filterSuppressedNextUp([
      mediaItem({ id: "ep-1", name: "Pilot" }),
      mediaItem({ id: "ep-2", name: "Nexus" }),
    ]);

    expect(filtered.map((item) => item.id)).toEqual(["ep-2"]);

    nowSpy.mockReturnValue(4_000);
    const afterExpiry = filterSuppressedNextUp([
      mediaItem({ id: "ep-1", name: "Pilot" }),
      mediaItem({ id: "ep-2", name: "Nexus" }),
    ]);

    expect(afterExpiry.map((item) => item.id)).toEqual(["ep-1", "ep-2"]);
  });

  it("removes completed episode and prepends next episode in cache", () => {
    const base = homepageCache({
      resume_items: [
        mediaItem({ id: "ep-1", name: "Pilot" }),
        mediaItem({ id: "ep-3", name: "Legacy" }),
      ],
      next_up_items: [mediaItem({ id: "ep-4", name: "Future" })],
    });

    const updated = applyEpisodeCompletionToHomepageCache(
      base,
      "ep-1",
      mediaItem({ id: "ep-2", name: "Nexus" }),
    );

    expect(updated.resume_items.map((item) => item.id)).toEqual(["ep-3"]);
    expect(updated.next_up_items.map((item) => item.id)).toEqual([
      "ep-2",
      "ep-4",
    ]);
  });

  it("does not duplicate next episode when it is already present", () => {
    const base = homepageCache({
      next_up_items: [
        mediaItem({ id: "ep-2", name: "Nexus" }),
        mediaItem({ id: "ep-3", name: "Another" }),
      ],
    });

    const updated = applyEpisodeCompletionToHomepageCache(
      base,
      "ep-1",
      mediaItem({ id: "ep-2", name: "Nexus" }),
    );

    expect(updated.next_up_items.map((item) => item.id)).toEqual([
      "ep-2",
      "ep-3",
    ]);
  });
});
