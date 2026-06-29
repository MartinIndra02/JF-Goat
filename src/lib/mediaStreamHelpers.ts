import type { MediaStreamInfo } from "./types";

export interface QualityOption {
  key: string;
  label: string;
  maxStreamingBitrate: number | null;
  targetHeight: number | null;
}

export const QUALITY_PRESETS = [
  { height: 2160, bitrate: 20_000_000 },
  { height: 1440, bitrate: 12_000_000 },
  { height: 1080, bitrate: 8_000_000 },
  { height: 720, bitrate: 5_000_000 },
  { height: 480, bitrate: 2_500_000 },
  { height: 360, bitrate: 1_500_000 },
  { height: 240, bitrate: 900_000 },
];

export function formatBitrate(bitrate: number): string {
  const mbps = bitrate / 1_000_000;
  return mbps >= 10 ? `${mbps.toFixed(0)} Mbps` : `${mbps.toFixed(1)} Mbps`;
}

export function generateQualityOptions(
  mediaStreams: MediaStreamInfo | null,
): QualityOption[] {
  const base: QualityOption[] = [
    {
      key: "direct-play",
      label: "Direct Play",
      maxStreamingBitrate: null,
      targetHeight: null,
    },
  ];

  if (!mediaStreams || !mediaStreams.video || mediaStreams.video.length === 0) {
    return base;
  }

  const sourceMaxHeight = mediaStreams.video
    .map((track) => track.height)
    .filter((height): height is number => typeof height === "number" && height > 0)
    .sort((a, b) => b - a)[0] ?? null;

  const filteredPresets = QUALITY_PRESETS
    .filter((preset) => sourceMaxHeight === null || preset.height <= sourceMaxHeight)
    .map((preset) => ({
      key: `preset-${preset.height}`,
      label: `${preset.height}p · ${formatBitrate(preset.bitrate)}`,
      maxStreamingBitrate: preset.bitrate,
      targetHeight: preset.height,
    }));

  return [...base, ...filteredPresets];
}
