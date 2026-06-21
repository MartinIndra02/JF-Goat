import { describe, expect, it } from "vitest";
import {
  backdropUrl,
  episodeThumbnailUrl,
  formatDate,
  formatRuntime,
  posterUrl,
  progressPercent,
  seasonNumber,
} from "./detailHelpers";
import type { MediaItem } from "../../lib/types";

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

describe("detailHelpers", () => {
  it("formats season numbers and runtime labels", () => {
    expect(seasonNumber("Season 3")).toBe("3");
    expect(seasonNumber(null)).toBe("?");

    expect(formatRuntime(1_200_000_000)).toBe("2m");
    expect(formatRuntime(54_000_000_000)).toBe("1h 30m");
    expect(formatRuntime(null)).toBe("");
  });

  it("formats valid dates and rejects invalid input", () => {
    expect(formatDate("2024-08-12T00:00:00.000Z")).toContain("2024");
    expect(formatDate(null)).toBe("");
  });

  it("caps progress percentage at 100", () => {
    const item = mediaItem({ run_time_ticks: 10, playback_ticks: 15 });
    expect(progressPercent(item)).toBe(100);
  });

  it("builds poster and backdrop URLs from available tags", () => {
    const withBackdrop = mediaItem({ id: "ep-1", backdrop_tag: "bg1" });
    expect(backdropUrl(withBackdrop)).toContain("/backdrop/ep-1?tag=bg1");

    const withSeries = mediaItem({ series_id: "series-1" });
    expect(backdropUrl(withSeries)).toContain(
      "/backdrop/series-1?tag=series-1",
    );

    const withPoster = mediaItem({ id: "ep-2", image_tag: "img2" });
    expect(posterUrl(withPoster)).toContain("/poster/ep-2?tag=img2");

    expect(episodeThumbnailUrl(withBackdrop)).toContain(
      "/backdrop/ep-1?tag=bg1",
    );
  });
});
