import { describe, expect, it, vi } from "vitest";
import { formatBitrate, generateQualityOptions } from "./mediaStreamHelpers";
import {
  resetPlayerState,
  getPlayerStatus,
  isPlayerVisible,
  getPlayerTitle,
  getPlayerItemId,
  getTimePos,
  getDuration,
  getRequestedAudioIndex,
  getRequestedSubtitleIndex,
  showPlayer,
} from "./stores/player.svelte";
import type { MediaStreamInfo } from "./types";

// Mock Tauri event API to prevent test runner errors
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("mediaStreamHelpers", () => {
  it("formats bitrates into human readable labels", () => {
    expect(formatBitrate(20_000_000)).toBe("20 Mbps");
    expect(formatBitrate(8_000_000)).toBe("8.0 Mbps");
    expect(formatBitrate(2_500_000)).toBe("2.5 Mbps");
    expect(formatBitrate(900_000)).toBe("0.9 Mbps");
  });

  it("generates correct quality options based on media streams", () => {
    // 1. Null stream info
    const optsNull = generateQualityOptions(null);
    expect(optsNull).toHaveLength(1);
    expect(optsNull[0].key).toBe("direct-play");

    // 2. Stream info with no video tracks
    const streamsNoVideo: MediaStreamInfo = {
      video: [],
      audio: [],
      subtitle: [],
      video_label: null,
    };
    const optsNoVideo = generateQualityOptions(streamsNoVideo);
    expect(optsNoVideo).toHaveLength(1);

    // 3. Stream info with video heights
    const streamsWithVideo: MediaStreamInfo = {
      video: [
        { index: 0, codec: "h264", display_title: "Video", language: null, is_default: true, height: 1080 },
        { index: 1, codec: "h264", display_title: "Video 2", language: null, is_default: false, height: 720 },
      ],
      audio: [],
      subtitle: [],
      video_label: "1080p SDR",
    };
    const opts = generateQualityOptions(streamsWithVideo);
    
    // Should include Direct Play + presets up to 1080p (1080p, 720p, 480p, 360p, 240p)
    expect(opts[0].key).toBe("direct-play");
    
    const presetHeights = opts.slice(1).map(o => o.targetHeight);
    expect(presetHeights).toEqual([1080, 720, 480, 360, 240]);
  });
});

describe("player store reset", () => {
  it("resets all transient player states successfully", () => {
    // Initialize player state with mock values
    showPlayer("item-123", "Cool Video");
    
    expect(getPlayerStatus()).toBe("loading");
    expect(isPlayerVisible()).toBe(true);
    expect(getPlayerItemId()).toBe("item-123");
    expect(getPlayerTitle()).toBe("Cool Video");

    // Reset player state
    resetPlayerState();

    // Verify all variables are reset
    expect(getPlayerStatus()).toBe("idle");
    expect(isPlayerVisible()).toBe(false);
    expect(getPlayerItemId()).toBeNull();
    expect(getPlayerTitle()).toBe("");
    expect(getTimePos()).toBe(0);
    expect(getDuration()).toBe(0);
    expect(getRequestedAudioIndex()).toBeNull();
    expect(getRequestedSubtitleIndex()).toBeNull();
  });
});
