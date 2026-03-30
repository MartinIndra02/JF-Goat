<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    isPlayerVisible,
    getPlayerStatus,
    getPlayerTitle,
    getPlayerItemId,
    getTimePos,
    getDuration,
    getVolume,
    isMuted,
    getPlaybackRate,
    hidePlayer,
    showPlayer,
    setVolume,
    setMuted,
    setPlaybackRate,
    setSubtitleTrack,
    setPreferredAudioStreamIndex,
    setPreferredSubtitleStreamIndex,
    setPreferredAudioMetadata,
    setPreferredSubtitleMetadata,
    getPreferredAudioStreamIndex,
    getPreferredSubtitleStreamIndex,
    resolvePreferredAudioStreamIndex,
    resolvePreferredSubtitleStreamIndex,
  } from "../../lib/stores/player.svelte";
  import {
    mpvPlay,
    mpvTogglePause,
    mpvSeek,
    mpvSeekAbsolute,
    mpvSetVolume,
    mpvSetMute,
    mpvSetPlaybackRate,
    mpvSetSubtitlePosition,
    mpvSetAudioTrack,
    mpvSetSubtitleTrack,
    mpvStop,
    reportPlaybackLifecycle,
    getItemById,
    getSeasonEpisodes,
    getMediaStreams,
    getItemChapters,
    loadHomepageCache,
    saveHomepageCache,
  } from "../../lib/api";
  import {
    applyEpisodeCompletionToHomepageCache,
    emitHomepageCacheUpdated,
    suppressNextUpItem,
  } from "../../lib/homepageFreshness";
  import type {
    ChapterInfo,
    MediaItem,
    MediaStreamInfo,
  } from "../../lib/types";

  // ── Auto-hide logic ──────────────────────────────────────────
  let controlsVisible = $state(true);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  function resetHideTimer() {
    controlsVisible = true;
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      controlsVisible = false;
    }, 3000);
  }

  function handleMouseMove() {
    resetHideTimer();
  }

  // ── UI state ─────────────────────────────────────────────────
  let isFullscreen = $state(false);
  let mediaStreams = $state<MediaStreamInfo | null>(null);
  let chapters = $state<ChapterInfo[]>([]);
  let previousEpisode = $state<MediaItem | null>(null);
  let nextEpisode = $state<MediaItem | null>(null);
  let autoplayCountdown = $state<number | null>(null);
  let autoplayTimer: ReturnType<typeof setInterval> | null = null;
  let playbackLifecycleTimer: ReturnType<typeof setInterval> | null = null;
  let autoplayDismissedForCurrentItem = $state(false);
  let autoplayStateItemId = $state<string | null>(null);
  let lifecycleStartedForItemId = $state<string | null>(null);
  let endAutoSkipHandledForItemId = $state<string | null>(null);
  let playbackContextItemId = $state<string | null>(null);
  let playbackContextResolvedItemId = $state<string | null>(null);
  let streamContextItemId = $state<string | null>(null);
  let deferredPlaybackContextTimer: ReturnType<typeof setTimeout> | null = null;
  let deferredStreamLoadTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingSeekClearTimer: ReturnType<typeof setTimeout> | null = null;
  let selectedAudioIndex = $state<number | null>(null);
  let selectedSubtitleIndex = $state<number | null>(null);
  let selectedQualityKey = $state("direct-play");
  let audioMenuOpen = $state(false);
  let subtitleMenuOpen = $state(false);
  let overflowMenuOpen = $state(false);
  let progressScrubEl = $state<HTMLElement | null>(null);
  let isScrubbing = $state(false);
  let scrubSeconds = $state<number | null>(null);
  let pendingSeekSeconds = $state<number | null>(null);

  const PLAYBACK_CONTEXT_DELAY_MS = 2500;
  const STREAM_CONTEXT_DELAY_MS = 3000;
  const PLAYBACK_PROGRESS_INTERVAL_MS = 15_000;
  const SUBTITLE_POSITION_STORAGE_KEY = "jfgoat.player.subtitleBottomPercent";
  const DEFAULT_SUBTITLE_POSITION_PERCENT = 92;
  const SUBTITLE_POSITION_CONTROLS_OFFSET = 14;

  function clampSubtitlePositionPercent(value: number): number {
    return Math.max(70, Math.min(98, Math.round(value)));
  }

  function getStoredSubtitlePositionPercent(): number {
    if (typeof localStorage === "undefined") {
      return DEFAULT_SUBTITLE_POSITION_PERCENT;
    }

    const raw = localStorage.getItem(SUBTITLE_POSITION_STORAGE_KEY);
    if (!raw) {
      return DEFAULT_SUBTITLE_POSITION_PERCENT;
    }

    const parsed = Number(raw);
    if (!Number.isFinite(parsed)) {
      return DEFAULT_SUBTITLE_POSITION_PERCENT;
    }

    return clampSubtitlePositionPercent(parsed);
  }

  // ── Derived values ───────────────────────────────────────────
  const playerVisible = $derived(isPlayerVisible());
  const playerStatus = $derived(getPlayerStatus());
  const playerTitle = $derived(getPlayerTitle());
  const playerItemId = $derived(getPlayerItemId());
  const pos = $derived(getTimePos());
  const dur = $derived(getDuration());
  const vol = $derived(getVolume());
  const muted = $derived(isMuted());
  const rate = $derived(getPlaybackRate());
  const effectivePos = $derived(scrubSeconds ?? pendingSeekSeconds ?? pos);
  const progressPercent = $derived(dur > 0 ? (effectivePos / dur) * 100 : 0);
  const isPaused = $derived(playerStatus === "paused");

  const chapterMarkers = $derived.by(() => {
    if (dur <= 0 || chapters.length === 0) return [];

    return chapters
      .map((chapter) => {
        const startSeconds = chapter.start_ticks / 10_000_000;
        const percent = Math.max(0, Math.min((startSeconds / dur) * 100, 100));
        return {
          ...chapter,
          startSeconds,
          percent,
        };
      })
      .filter((chapter) => chapter.percent > 0 && chapter.percent < 100);
  });

  const outroStartSeconds = $derived.by(() => {
    if (dur <= 0 || chapters.length === 0) return null;

    const looksLikeOutro = (chapter: ChapterInfo): boolean => {
      const name = (chapter.name ?? "").toLowerCase();
      const marker = (chapter.marker_type ?? "").toLowerCase();
      const chapterType = (chapter.chapter_type ?? "").toLowerCase();

      return (
        name.includes("outro")
        || name.includes("credits")
        || name.includes("credit")
        || name.includes("end")
        || marker.includes("outro")
        || marker.includes("credit")
        || chapterType.includes("outro")
        || chapterType.includes("credit")
      );
    };

    const nearTailStartTicks = Math.floor(dur * 0.55 * 10_000_000);

    const candidates = chapters
      .filter(looksLikeOutro)
      .map((chapter) => chapter.start_ticks)
      .filter((ticks) => ticks >= nearTailStartTicks)
      .sort((a, b) => a - b);

    if (candidates.length === 0) return null;
    return candidates[0] / 10_000_000;
  });

  const isInOutroWindow = $derived.by(() => {
    if (!nextEpisode || !outroStartSeconds || dur <= 0) return false;
    return pos >= outroStartSeconds && pos < dur;
  });

  const audioLabel = $derived.by(() => {
    if (!mediaStreams || mediaStreams.audio.length === 0) {
      return "Default audio";
    }

    if (selectedAudioIndex === null) {
      return "Default audio";
    }

    const track = mediaStreams.audio.find((s) => s.index === selectedAudioIndex);
    return track?.display_title ?? `Audio ${selectedAudioIndex}`;
  });

  const subtitleLabel = $derived.by(() => {
    if (!mediaStreams || mediaStreams.subtitle.length === 0) {
      return "No subtitles";
    }

    if (selectedSubtitleIndex === null) {
      return "Subtitles Off";
    }

    const track = mediaStreams.subtitle.find((s) => s.index === selectedSubtitleIndex);
    return track?.display_title ?? `Subtitle ${selectedSubtitleIndex}`;
  });

  const selectedAudioTrack = $derived.by(() =>
    mediaStreams?.audio.find((s) => s.index === selectedAudioIndex) ?? null,
  );

  const selectedSubtitleTrack = $derived.by(() =>
    mediaStreams?.subtitle.find((s) => s.index === selectedSubtitleIndex) ?? null,
  );

  const qualityOptions = $derived.by(() => {
    const base = [
      {
        key: "direct-play",
        label: "Direct Play",
        maxStreamingBitrate: null,
        targetHeight: null,
      },
    ];

    if (!mediaStreams || mediaStreams.video.length === 0) {
      return base;
    }

    const formatBitrate = (bitrate: number): string => {
      const mbps = bitrate / 1_000_000;
      return mbps >= 10 ? `${mbps.toFixed(0)} Mbps` : `${mbps.toFixed(1)} Mbps`;
    };

    const sourceMaxHeight = mediaStreams.video
      .map((track) => track.height)
      .filter((height): height is number => typeof height === "number" && height > 0)
      .sort((a, b) => b - a)[0] ?? null;

    const presets = [
      { height: 2160, bitrate: 20_000_000 },
      { height: 1440, bitrate: 12_000_000 },
      { height: 1080, bitrate: 8_000_000 },
      { height: 720, bitrate: 5_000_000 },
      { height: 480, bitrate: 2_500_000 },
      { height: 360, bitrate: 1_500_000 },
      { height: 240, bitrate: 900_000 },
    ];

    const filteredPresets = presets
      .filter((preset) => sourceMaxHeight === null || preset.height <= sourceMaxHeight)
      .map((preset) => ({
        key: `preset-${preset.height}`,
        label: `${preset.height}p · ${formatBitrate(preset.bitrate)}`,
        maxStreamingBitrate: preset.bitrate,
        targetHeight: preset.height,
      }));

    return [...base, ...filteredPresets];
  });

  const selectedQuality = $derived.by(() => {
    return qualityOptions.find((option) => option.key === selectedQualityKey)
      ?? qualityOptions[0];
  });

  const selectedQualityLabel = $derived.by(() => selectedQuality.label);

  const audioMenuLabel = $derived.by(() => {
    const language = selectedAudioTrack?.language?.trim();
    if (language) return language.toUpperCase();
    if (selectedAudioTrack?.display_title) return selectedAudioTrack.display_title;
    return "Default";
  });

  const subtitleMenuLabel = $derived.by(() => {
    if (selectedSubtitleIndex === null) return "Off";
    const language = selectedSubtitleTrack?.language?.trim();
    if (language) return language.toUpperCase();
    if (selectedSubtitleTrack?.display_title) return selectedSubtitleTrack.display_title;
    return "On";
  });

  // ── Formatting helpers ───────────────────────────────────────
  function formatTime(seconds: number): string {
    if (!seconds || seconds < 0) return "0:00";
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    const pad = (n: number) => n.toString().padStart(2, "0");
    return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
  }

  function endTimeEstimate(): string {
    const remaining = dur - effectivePos;
    if (remaining <= 0) return "";
    const end = new Date(Date.now() + remaining * 1000);
    return `Ends at ${end.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  }

  function formatEpisodeTitle(item: MediaItem): string {
    if (item.type === "Episode" && item.series_name) {
      const season = item.season_name?.match(/(\d+)/)?.[1] ?? "?";
      const episode = item.index_number ?? "?";
      return `${item.series_name} - S${season} E${episode} - ${item.name}`;
    }
    return item.name;
  }

  // ── Playback context loading ─────────────────────────────────
  async function resolveEpisodeNeighbors(item: MediaItem): Promise<{
    previous: MediaItem | null;
    next: MediaItem | null;
  }> {
    if (item.type !== "Episode" || !item.season_id) {
      return {
        previous: null,
        next: null,
      };
    }

    try {
      const episodes = await getSeasonEpisodes(item.season_id);
      if (episodes.length === 0) {
        return {
          previous: null,
          next: null,
        };
      }

      const ordered = [...episodes].sort((a, b) => {
        const ai = a.index_number ?? 0;
        const bi = b.index_number ?? 0;
        return ai - bi;
      });

      const currentIndex = ordered.findIndex((ep) => ep.id === item.id);
      if (currentIndex < 0) {
        return {
          previous: null,
          next: null,
        };
      }

      return {
        previous: ordered[currentIndex - 1] ?? null,
        next: ordered[currentIndex + 1] ?? null,
      };
    } catch {
      return {
        previous: null,
        next: null,
      };
    }
  }

  async function loadPlaybackContext(id: string) {
    const [item, chapterList] = await Promise.all([
      getItemById(id).catch(() => null),
      getItemChapters(id).catch(() => []),
    ]);

    if (playerItemId !== id) return;

    playbackContextResolvedItemId = id;
    chapters = chapterList;

    if (item) {
      const neighbors = await resolveEpisodeNeighbors(item);

      if (playerItemId !== id) return;

      previousEpisode = neighbors.previous;
      nextEpisode = neighbors.next;
    } else {
      previousEpisode = null;
      nextEpisode = null;
    }
  }

  async function loadStreamContext(id: string) {
    streamContextItemId = id;
    const streams = await getMediaStreams(id).catch(() => null);

    if (playerItemId !== id) return;

    mediaStreams = streams;
    if (streams) {
      selectedAudioIndex = resolvePreferredAudioStreamIndex(streams.audio);
      selectedSubtitleIndex = resolvePreferredSubtitleStreamIndex(streams.subtitle);
      setSubtitleTrack(selectedSubtitleIndex);
    } else {
      selectedAudioIndex = null;
      selectedSubtitleIndex = null;
      setSubtitleTrack(null);
    }
  }

  function schedulePlaybackContextLoad(id: string) {
    if (deferredPlaybackContextTimer) {
      clearTimeout(deferredPlaybackContextTimer);
    }

    deferredPlaybackContextTimer = setTimeout(() => {
      deferredPlaybackContextTimer = null;
      void loadPlaybackContext(id);
    }, PLAYBACK_CONTEXT_DELAY_MS);
  }

  function scheduleStreamContextLoad(id: string) {
    if (deferredStreamLoadTimer) {
      clearTimeout(deferredStreamLoadTimer);
    }

    deferredStreamLoadTimer = setTimeout(() => {
      deferredStreamLoadTimer = null;
      void loadStreamContext(id);
    }, STREAM_CONTEXT_DELAY_MS);
  }

  function ensureStreamContextLoadedNow() {
    if (!playerItemId) return;
    if (mediaStreams || streamContextItemId === playerItemId) return;

    if (deferredStreamLoadTimer) {
      clearTimeout(deferredStreamLoadTimer);
      deferredStreamLoadTimer = null;
    }

    void loadStreamContext(playerItemId);
  }

  function clearPendingSeekPreview() {
    pendingSeekSeconds = null;
    if (pendingSeekClearTimer) {
      clearTimeout(pendingSeekClearTimer);
      pendingSeekClearTimer = null;
    }
  }

  function closeTopMenus() {
    audioMenuOpen = false;
    subtitleMenuOpen = false;
    overflowMenuOpen = false;
  }

  function toggleTopMenu(menu: "audio" | "subtitle" | "overflow") {
    const openAudio = menu === "audio" ? !audioMenuOpen : false;
    const openSubtitle = menu === "subtitle" ? !subtitleMenuOpen : false;
    const openOverflow = menu === "overflow" ? !overflowMenuOpen : false;

    audioMenuOpen = openAudio;
    subtitleMenuOpen = openSubtitle;
    overflowMenuOpen = openOverflow;

    if (openAudio || openSubtitle || openOverflow) {
      ensureStreamContextLoadedNow();
    }
  }

  function subtitleIndexForRequest(value: number | null | undefined): number | null {
    if (value === undefined) return null;
    if (value === null) return -1;
    return value;
  }

  function isNearPlaybackEnd(positionSeconds: number, durationSeconds: number): boolean {
    if (durationSeconds <= 0) return false;

    const remaining = Math.max(durationSeconds - positionSeconds, 0);
    const remainingThreshold = 60;
    const percent = positionSeconds / durationSeconds;

    return remaining <= remainingThreshold || percent >= 0.95;
  }

  async function applyOptimisticHomepageUpdate(
    completedEpisodeId: string,
    nextEpisodeHint: MediaItem | null,
  ) {
    suppressNextUpItem(completedEpisodeId);

    try {
      const cached = await loadHomepageCache();
      if (!cached) return;

      const updated = applyEpisodeCompletionToHomepageCache(
        cached,
        completedEpisodeId,
        nextEpisodeHint,
      );
      await saveHomepageCache(updated);
      emitHomepageCacheUpdated(updated);
    } catch (e) {
      console.warn("Failed to optimistically update homepage cache:", e);
    }
  }

  async function reportCurrentPlaybackStop(nextEpisodeHint: MediaItem | null = null) {
    if (!playerItemId) return;

    const currentItemId = playerItemId;
    const positionSeconds = Math.max(0, pos);
    const durationSeconds = Math.max(0, dur);
    const positionTicks = Math.floor(positionSeconds * 10_000_000);
    const durationTicks = Math.floor(durationSeconds * 10_000_000);
    const nearEnd = isNearPlaybackEnd(positionSeconds, durationSeconds);

    try {
      await reportPlaybackLifecycle(
        currentItemId,
        positionTicks,
        durationTicks,
        "stopped",
      );
    } catch (e) {
      console.warn("Failed to report playback stop:", e);
      return;
    }

    if (nearEnd) {
      await applyOptimisticHomepageUpdate(currentItemId, nextEpisodeHint);
    }
  }

  async function reportPlaybackHeartbeat(event: "playing" | "progress") {
    if (!playerItemId) return;

    const positionTicks = Math.floor(Math.max(0, pos) * 10_000_000);
    const durationTicks = Math.floor(Math.max(0, dur) * 10_000_000);

    try {
      await reportPlaybackLifecycle(
        playerItemId,
        positionTicks,
        durationTicks,
        event,
      );
    } catch (e) {
      console.warn(`Failed to report playback ${event}:`, e);
    }
  }

  function stopPlaybackLifecycleTimer() {
    if (playbackLifecycleTimer) {
      clearInterval(playbackLifecycleTimer);
      playbackLifecycleTimer = null;
    }
  }

  async function applyQualitySelection(nextQualityKey: string) {
    if (!playerItemId) return;

    const quality = qualityOptions.find((option) => option.key === nextQualityKey);
    if (!quality) return;

    selectedQualityKey = nextQualityKey;
    await mpvPlay({
      itemId: playerItemId,
      startTicks: Math.max(0, Math.floor(pos * 10_000_000)),
      audioStreamIndex: selectedAudioIndex,
      subtitleStreamIndex: subtitleIndexForRequest(selectedSubtitleIndex),
      maxStreamingBitrate: quality.maxStreamingBitrate,
      targetHeight: quality.targetHeight,
    });
  }

  function mapAudioIndexToMpvId(streams: MediaStreamInfo, streamIndex: number): number | null {
    let mpvId = 1;
    for (const track of streams.audio) {
      if (track.index === streamIndex) {
        return mpvId;
      }
      if (!track.is_external) {
        mpvId += 1;
      }
    }
    return null;
  }

  function mapSubtitleIndexToMpvId(streams: MediaStreamInfo, streamIndex: number): number | null {
    let mpvId = 1;
    for (const track of streams.subtitle) {
      if (track.index === streamIndex) {
        const deliveryMethod = (track.delivery_method ?? "").toLowerCase();
        if (deliveryMethod === "embed" || deliveryMethod === "") {
          return mpvId;
        }
        return null;
      }
      if (!track.is_external) {
        mpvId += 1;
      }
    }
    return null;
  }

  async function applyTrackSelection(
    nextAudioIndex: number | null,
    nextSubtitleIndex: number | null,
  ) {
    if (!playerItemId) return;

    selectedAudioIndex = nextAudioIndex;
    selectedSubtitleIndex = nextSubtitleIndex;
    setSubtitleTrack(nextSubtitleIndex);

    if (nextAudioIndex !== null) {
      setPreferredAudioStreamIndex(nextAudioIndex);
    }
    setPreferredSubtitleStreamIndex(nextSubtitleIndex);

    const selectedAudioTrack = mediaStreams?.audio.find(
      (track) => track.index === nextAudioIndex,
    );
    const selectedSubtitleTrack = mediaStreams?.subtitle.find(
      (track) => track.index === nextSubtitleIndex,
    );
    setPreferredAudioMetadata(
      selectedAudioTrack?.language,
      selectedAudioTrack?.display_title,
    );
    setPreferredSubtitleMetadata(
      selectedSubtitleTrack?.language,
      selectedSubtitleTrack?.display_title,
    );

    let requireStreamReload = false;

    if (mediaStreams && nextAudioIndex !== null) {
      const mpvAudioId = mapAudioIndexToMpvId(mediaStreams, nextAudioIndex);
      if (mpvAudioId !== null) {
        await mpvSetAudioTrack(mpvAudioId);
      } else {
        requireStreamReload = true;
      }
    } else if (nextAudioIndex !== null) {
      requireStreamReload = true;
    }

    if (nextSubtitleIndex === null) {
      await mpvSetSubtitleTrack(null);
    } else if (mediaStreams) {
      const mpvSubtitleId = mapSubtitleIndexToMpvId(mediaStreams, nextSubtitleIndex);
      if (mpvSubtitleId !== null) {
        await mpvSetSubtitleTrack(mpvSubtitleId);
      } else {
        requireStreamReload = true;
      }
    } else {
      requireStreamReload = true;
    }

    if (requireStreamReload) {
      // Keep the active quality profile when track changes force a stream reload.
      const quality = selectedQuality;
      await mpvPlay({
        itemId: playerItemId,
        startTicks: Math.max(0, Math.floor(pos * 10_000_000)),
        audioStreamIndex: nextAudioIndex,
        subtitleStreamIndex: subtitleIndexForRequest(nextSubtitleIndex),
        maxStreamingBitrate: quality.maxStreamingBitrate,
        targetHeight: quality.targetHeight,
      });
    }
  }

  // ── Autoplay countdown ───────────────────────────────────────
  function stopAutoplayCountdown() {
    if (autoplayTimer) {
      clearInterval(autoplayTimer);
      autoplayTimer = null;
    }
    autoplayCountdown = null;
  }

  function cancelAutoplayCountdown() {
    autoplayDismissedForCurrentItem = true;
    stopAutoplayCountdown();
  }

  function startAutoplayCountdown() {
    if (!nextEpisode || autoplayDismissedForCurrentItem || autoplayCountdown !== null) return;

    stopAutoplayCountdown();
    autoplayCountdown = 10;

    autoplayTimer = setInterval(() => {
      if (autoplayCountdown === null) return;

      if (autoplayCountdown <= 1) {
        stopAutoplayCountdown();
        void playNextEpisode();
        return;
      }

      autoplayCountdown -= 1;
    }, 1000);
  }

  async function playNextEpisode() {
    if (!nextEpisode) return;

    const target = nextEpisode;
    void reportCurrentPlaybackStop(target);
    stopAutoplayCountdown();

    showPlayer(target.id, formatEpisodeTitle(target));

    try {
      const preferredAudio = getPreferredAudioStreamIndex() ?? selectedAudioIndex;
      const preferredSubtitle = getPreferredSubtitleStreamIndex() ?? selectedSubtitleIndex;

      await mpvPlay({
        itemId: target.id,
        startTicks: 0,
        audioStreamIndex: preferredAudio,
        subtitleStreamIndex: subtitleIndexForRequest(preferredSubtitle),
      });
    } catch (e) {
      console.error("Failed to autoplay next episode:", e);
    }
  }

  // ── Control handlers ─────────────────────────────────────────
  async function togglePause() {
    await mpvTogglePause();
  }

  async function seekBack10() {
    await mpvSeek(-10);
  }

  async function seekForward30() {
    await mpvSeek(30);
  }

  async function seekToChapter(seconds: number) {
    await mpvSeekAbsolute(seconds);
  }

  function secondsFromPointer(clientX: number): number | null {
    if (!progressScrubEl || dur <= 0) return null;

    const rect = progressScrubEl.getBoundingClientRect();
    if (rect.width <= 0) return null;

    const fraction = Math.max(0, Math.min((clientX - rect.left) / rect.width, 1));
    return fraction * dur;
  }

  function beginTimelineScrub(e: PointerEvent) {
    if (dur <= 0) return;

    const seconds = secondsFromPointer(e.clientX);
    if (seconds === null) return;

    e.preventDefault();
    e.stopPropagation();

    clearPendingSeekPreview();
    isScrubbing = true;
    scrubSeconds = seconds;
    controlsVisible = true;

    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
  }

  function handleWindowPointerMove(e: PointerEvent) {
    if (!isScrubbing) return;

    const seconds = secondsFromPointer(e.clientX);
    if (seconds !== null) {
      scrubSeconds = seconds;
    }
  }

  function endTimelineScrub(e: PointerEvent) {
    if (!isScrubbing) return;

    const seconds = secondsFromPointer(e.clientX) ?? scrubSeconds;
    isScrubbing = false;
    scrubSeconds = null;

    if (seconds !== null) {
      pendingSeekSeconds = seconds;
      scrubSeconds = null;
      if (pendingSeekClearTimer) {
        clearTimeout(pendingSeekClearTimer);
      }
      // Keep previewed seek position long enough for mpv to settle and report the new time.
      pendingSeekClearTimer = setTimeout(() => {
        pendingSeekSeconds = null;
        pendingSeekClearTimer = null;
      }, 6000);
      void mpvSeekAbsolute(seconds);
    } else {
      scrubSeconds = null;
    }

    resetHideTimer();
  }

  function cancelTimelineScrub() {
    if (!isScrubbing) return;

    isScrubbing = false;
    scrubSeconds = null;
    resetHideTimer();
  }

  function handleProgressKeydown(e: KeyboardEvent) {
    if (!dur) return;

    if (e.key === "ArrowLeft") {
      e.preventDefault();
      void mpvSeek(-10);
      return;
    }

    if (e.key === "ArrowRight") {
      e.preventDefault();
      void mpvSeek(10);
      return;
    }

    if (e.key === "Home") {
      e.preventDefault();
      void mpvSeekAbsolute(0);
      return;
    }

    if (e.key === "End") {
      e.preventDefault();
      void mpvSeekAbsolute(dur);
    }
  }

  async function playPreviousEpisode() {
    if (!previousEpisode) return;

    void reportCurrentPlaybackStop();

    const target = previousEpisode;
    stopAutoplayCountdown();

    showPlayer(target.id, formatEpisodeTitle(target));

    try {
      const preferredAudio = getPreferredAudioStreamIndex() ?? selectedAudioIndex;
      const preferredSubtitle = getPreferredSubtitleStreamIndex() ?? selectedSubtitleIndex;

      await mpvPlay({
        itemId: target.id,
        startTicks: 0,
        audioStreamIndex: preferredAudio,
        subtitleStreamIndex: subtitleIndexForRequest(preferredSubtitle),
      });
    } catch (e) {
      console.error("Failed to play previous episode:", e);
    }
  }

  async function handleVolumeInput(e: Event) {
    const v = parseFloat((e.target as HTMLInputElement).value);
    setVolume(v);
    await mpvSetVolume(v);
  }

  async function toggleMute() {
    const nextMuted = !muted;
    setMuted(nextMuted);
    await mpvSetMute(nextMuted);
  }

  async function setRate(nextRate: number) {
    setPlaybackRate(nextRate);
    await mpvSetPlaybackRate(nextRate);
  }

  async function handleSubtitleChoice(nextTrack: number | null) {
    closeTopMenus();
    await applyTrackSelection(selectedAudioIndex, nextTrack);
  }

  async function handleAudioChoice(nextTrack: number) {
    closeTopMenus();
    await applyTrackSelection(nextTrack, selectedSubtitleIndex);
  }

  function syncFullscreenState() {
    isFullscreen = !!document.fullscreenElement;
  }

  async function exitFullscreenIfActive() {
    if (!document.fullscreenElement) {
      isFullscreen = false;
      return;
    }

    try {
      await document.exitFullscreen();
    } catch (e) {
      console.warn("Failed to exit fullscreen", e);
    } finally {
      syncFullscreenState();
    }
  }

  async function stopPlayer(nextEpisodeHint: MediaItem | null = null) {
    stopAutoplayCountdown();
    await reportCurrentPlaybackStop(nextEpisodeHint);
    await exitFullscreenIfActive();
    await mpvStop();
    hidePlayer();
  }

  async function toggleFullscreen() {
    try {
      if (!document.fullscreenElement) {
        await document.documentElement.requestFullscreen();
      } else {
        await document.exitFullscreen();
      }
      syncFullscreenState();
    } catch (e) {
      console.warn("Fullscreen toggle failed", e);
    }
  }

  // ── Keyboard shortcuts ───────────────────────────────────────
  function handleKeydown(e: KeyboardEvent) {
    if (!playerVisible) return;

    switch (e.key) {
      case " ":
        e.preventDefault();
        void togglePause();
        break;
      case "ArrowLeft":
        void mpvSeek(-10);
        break;
      case "ArrowRight":
        void mpvSeek(30);
        break;
      case "ArrowUp": {
        e.preventDefault();
        const newVolUp = Math.min(vol + 5, 100);
        setVolume(newVolUp);
        void mpvSetVolume(newVolUp);
        break;
      }
      case "ArrowDown": {
        e.preventDefault();
        const newVolDown = Math.max(vol - 5, 0);
        setVolume(newVolDown);
        void mpvSetVolume(newVolDown);
        break;
      }
      case "Escape":
        if (isFullscreen) {
          void toggleFullscreen();
        } else {
          void stopPlayer();
        }
        break;
      case "m":
      case "M":
        void toggleMute();
        break;
      case "f":
      case "F":
        void toggleFullscreen();
        break;
      case "[": {
        e.preventDefault();
        const nextRateDown = Math.max(rate - 0.1, 0.5);
        setPlaybackRate(nextRateDown);
        void mpvSetPlaybackRate(nextRateDown);
        break;
      }
      case "]": {
        e.preventDefault();
        const nextRateUp = Math.min(rate + 0.1, 2);
        setPlaybackRate(nextRateUp);
        void mpvSetPlaybackRate(nextRateUp);
        break;
      }
    }

    resetHideTimer();
  }

  onMount(() => {
    resetHideTimer();
    syncFullscreenState();

    const onFullscreenChange = () => syncFullscreenState();
    document.addEventListener("fullscreenchange", onFullscreenChange);

    return () => {
      document.removeEventListener("fullscreenchange", onFullscreenChange);
    };
  });

  onDestroy(() => {
    if (hideTimer) clearTimeout(hideTimer);
    stopPlaybackLifecycleTimer();
    if (pendingSeekClearTimer) clearTimeout(pendingSeekClearTimer);
    if (deferredPlaybackContextTimer) clearTimeout(deferredPlaybackContextTimer);
    if (deferredStreamLoadTimer) clearTimeout(deferredStreamLoadTimer);
    stopAutoplayCountdown();
  });

  $effect(() => {
    if (!playerVisible || !playerItemId) {
      void exitFullscreenIfActive();
      stopPlaybackLifecycleTimer();
      mediaStreams = null;
      chapters = [];
      previousEpisode = null;
      nextEpisode = null;
      isScrubbing = false;
      scrubSeconds = null;
      clearPendingSeekPreview();
      selectedAudioIndex = null;
      selectedSubtitleIndex = null;
      selectedQualityKey = "direct-play";
      playbackContextItemId = null;
      playbackContextResolvedItemId = null;
      streamContextItemId = null;
      if (deferredStreamLoadTimer) {
        clearTimeout(deferredStreamLoadTimer);
        deferredStreamLoadTimer = null;
      }
      if (deferredPlaybackContextTimer) {
        clearTimeout(deferredPlaybackContextTimer);
        deferredPlaybackContextTimer = null;
      }
      closeTopMenus();
      stopAutoplayCountdown();
      autoplayDismissedForCurrentItem = false;
      autoplayStateItemId = null;
      lifecycleStartedForItemId = null;
      endAutoSkipHandledForItemId = null;
      return;
    }

    if (autoplayStateItemId !== playerItemId) {
      autoplayStateItemId = playerItemId;
      autoplayDismissedForCurrentItem = false;
      endAutoSkipHandledForItemId = null;
    }

    if (playbackContextItemId !== playerItemId) {
      playbackContextItemId = playerItemId;
      lifecycleStartedForItemId = null;
      playbackContextResolvedItemId = null;
      mediaStreams = null;
      streamContextItemId = null;
      selectedQualityKey = "direct-play";
      if (deferredStreamLoadTimer) {
        clearTimeout(deferredStreamLoadTimer);
        deferredStreamLoadTimer = null;
      }
      if (deferredPlaybackContextTimer) {
        clearTimeout(deferredPlaybackContextTimer);
        deferredPlaybackContextTimer = null;
      }
      schedulePlaybackContextLoad(playerItemId);
    }
  });

  $effect(() => {
    if (!playerVisible || !playerItemId) return;
    if (playerStatus !== "playing") return;
    if (mediaStreams || streamContextItemId === playerItemId) return;
    if (deferredStreamLoadTimer) return;

    scheduleStreamContextLoad(playerItemId);
  });

  $effect(() => {
    if (!playerVisible || !playerItemId || playerStatus !== "playing") {
      stopPlaybackLifecycleTimer();
      return;
    }

    if (lifecycleStartedForItemId !== playerItemId) {
      lifecycleStartedForItemId = playerItemId;
      void reportPlaybackHeartbeat("playing");
    }

    stopPlaybackLifecycleTimer();
    playbackLifecycleTimer = setInterval(() => {
      if (!playerVisible || !playerItemId || playerStatus !== "playing") return;
      void reportPlaybackHeartbeat("progress");
    }, PLAYBACK_PROGRESS_INTERVAL_MS);

    return () => {
      stopPlaybackLifecycleTimer();
    };
  });

  $effect(() => {
    if (!playerVisible) {
      stopAutoplayCountdown();
      return;
    }

    if (playerStatus === "ended") {
      stopAutoplayCountdown();
      if (playerItemId && endAutoSkipHandledForItemId !== playerItemId) {
        // Playback context can still be loading near EOF. Resolve once before
        // deciding there is no next episode.
        if (!nextEpisode && playbackContextResolvedItemId !== playerItemId) {
          if (deferredPlaybackContextTimer) {
            clearTimeout(deferredPlaybackContextTimer);
            deferredPlaybackContextTimer = null;
          }
          void loadPlaybackContext(playerItemId);
          return;
        }

        endAutoSkipHandledForItemId = playerItemId;
        if (nextEpisode && !autoplayDismissedForCurrentItem) {
          void playNextEpisode();
        } else if (nextEpisode && autoplayDismissedForCurrentItem) {
          void stopPlayer(nextEpisode);
        } else {
          void stopPlayer();
        }
      }
      return;
    }

    if (!nextEpisode) {
      stopAutoplayCountdown();
      return;
    }

    if (playerStatus !== "playing") {
      stopAutoplayCountdown();
      return;
    }

    if (isInOutroWindow && !autoplayDismissedForCurrentItem) {
      startAutoplayCountdown();
    } else {
      stopAutoplayCountdown();
    }
  });

  $effect(() => {
    if (pendingSeekSeconds === null || isScrubbing) return;

    if (Math.abs(pos - pendingSeekSeconds) <= 0.6) {
      clearPendingSeekPreview();
    }
  });

  $effect(() => {
    if (!qualityOptions.some((option) => option.key === selectedQualityKey)) {
      selectedQualityKey = qualityOptions[0]?.key ?? "direct-play";
    }
  });

  $effect(() => {
    if (!playerVisible) return;

    if (!controlsVisible) {
      closeTopMenus();
    }

    const storedSubtitlePosition = getStoredSubtitlePositionPercent();
    const subtitlePosition = controlsVisible
      ? clampSubtitlePositionPercent(
        storedSubtitlePosition - SUBTITLE_POSITION_CONTROLS_OFFSET,
      )
      : storedSubtitlePosition;
    void mpvSetSubtitlePosition(subtitlePosition);
  });
</script>

<svelte:window
  onkeydown={handleKeydown}
  onpointermove={handleWindowPointerMove}
  onpointerup={endTimelineScrub}
  onpointercancel={cancelTimelineScrub}
/>

{#if playerVisible}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-[9999] flex flex-col justify-between"
    style:background-color={playerStatus === "loading" ? "black" : "transparent"}
    class:cursor-none={!controlsVisible}
    onmousemove={handleMouseMove}
  >
    <div
      class="relative z-10 px-3 sm:px-6 pt-3 sm:pt-5 pb-10 bg-gradient-to-b from-black/80 via-black/45 to-transparent transition-all duration-300 ease-out"
      class:opacity-0={!controlsVisible}
      class:-translate-y-full={!controlsVisible}
    >
      <div class="mx-auto w-full max-w-6xl flex items-center justify-between gap-3">
        <div class="flex items-center gap-2 sm:gap-3 min-w-0">
          <button
            onclick={() => void stopPlayer()}
            aria-label="Close player"
            class="h-10 w-10 grid place-items-center rounded-xl bg-black/45 border border-white/20 backdrop-blur-md text-white hover:bg-black/60 transition-colors"
          >
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7" />
            </svg>
          </button>

          <div class="min-w-0 rounded-xl bg-black/40 border border-white/15 px-3 py-2 backdrop-blur-md">
            <p class="text-white text-sm sm:text-base font-semibold truncate">{playerTitle}</p>
            <p class="text-[11px] text-gray-300 truncate">{subtitleLabel}</p>
          </div>
        </div>

        <button
            onclick={toggleFullscreen}
            aria-label={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
            class="h-10 w-10 grid place-items-center rounded-xl bg-black/45 border border-white/20 backdrop-blur-md text-white hover:bg-black/60 transition-colors"
          >
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M8 3H5a2 2 0 00-2 2v3m16-5h-3m3 0v3M3 16v3a2 2 0 002 2h3m11-5v3a2 2 0 01-2 2h-3"/>
            </svg>
          </button>
      </div>
    </div>

    <button
      class="absolute inset-0 z-0 w-full h-full"
      onclick={togglePause}
      aria-label={isPaused ? "Resume playback" : "Pause playback"}
    ></button>

    <div
      class="relative z-10 px-3 sm:px-6 pb-2 sm:pb-3 pt-10 sm:pt-12 bg-gradient-to-t from-black/85 via-black/50 to-transparent transition-all duration-300 ease-out"
      class:opacity-0={!controlsVisible}
      class:translate-y-full={!controlsVisible}
    >
      <div class="mx-auto w-full max-w-6xl rounded-xl border border-white/15 bg-black/45 backdrop-blur-xl shadow-[0_12px_30px_rgba(0,0,0,0.42)] p-2.5 sm:p-3">
        <div class="flex flex-wrap items-center justify-between gap-2 sm:gap-2.5 mb-2">
          <div class="min-w-0">
            <p class="text-white text-sm font-semibold truncate">{playerTitle}</p>
            <div class="flex flex-wrap items-center gap-1.5 mt-1 text-[10px] sm:text-[11px]">
              <span class="px-2 py-0.5 rounded-full bg-cyan-400/15 text-cyan-200 border border-cyan-300/20">{selectedQualityLabel}</span>
              <span class="px-2 py-0.5 rounded-full bg-white/10 text-gray-200 border border-white/15 truncate max-w-[220px]">{audioLabel}</span>
              <span class="px-2 py-0.5 rounded-full bg-white/10 text-gray-200 border border-white/15 truncate max-w-[220px]">{subtitleLabel}</span>
              {#if endTimeEstimate()}
                <span class="text-gray-300">{endTimeEstimate()}</span>
              {/if}
            </div>
          </div>

          <div class="flex items-center gap-2 flex-wrap justify-end">
            <div class="flex items-center gap-1.5">
              {#if mediaStreams && mediaStreams.audio.length > 0}
                <div class="relative">
                  <button
                    onclick={() => toggleTopMenu("audio")}
                    aria-label="Select audio language"
                    class="h-8 px-2.5 rounded-lg bg-black/50 border border-white/20 backdrop-blur-md text-white hover:bg-black/65 transition-colors inline-flex items-center gap-1.5 max-w-[170px]"
                  >
                    <span class="text-[10px] uppercase tracking-wide text-cyan-200">Lang</span>
                    <span class="text-[11px] truncate">{audioMenuLabel}</span>
                  </button>

                  {#if audioMenuOpen}
                    <div class="absolute right-0 bottom-10 w-64 max-h-72 overflow-auto rounded-xl border border-white/20 bg-black/82 backdrop-blur-xl shadow-[0_18px_48px_rgba(0,0,0,0.55)] p-1.5">
                      {#each mediaStreams.audio as track}
                        <button
                          type="button"
                          onclick={() => handleAudioChoice(track.index)}
                          class="w-full text-left px-3 py-2 rounded-lg text-xs text-gray-100 hover:bg-white/15 transition-colors"
                          class:bg-cyan-400={selectedAudioIndex === track.index}
                          class:bg-opacity-20={selectedAudioIndex === track.index}
                        >
                          {track.display_title}
                        </button>
                      {/each}
                    </div>
                  {/if}
                </div>
              {/if}

              {#if mediaStreams && mediaStreams.subtitle.length > 0}
                <div class="relative">
                  <button
                    onclick={() => toggleTopMenu("subtitle")}
                    aria-label="Select subtitle language"
                    class="h-8 px-2.5 rounded-lg bg-black/50 border border-white/20 backdrop-blur-md text-white hover:bg-black/65 transition-colors inline-flex items-center gap-1.5 max-w-[170px]"
                  >
                    <span class="text-[10px] uppercase tracking-wide text-emerald-200">Subs</span>
                    <span class="text-[11px] truncate">{subtitleMenuLabel}</span>
                  </button>

                  {#if subtitleMenuOpen}
                    <div class="absolute right-0 bottom-10 w-64 max-h-72 overflow-auto rounded-xl border border-white/20 bg-black/82 backdrop-blur-xl shadow-[0_18px_48px_rgba(0,0,0,0.55)] p-1.5">
                      <button
                        type="button"
                        onclick={() => handleSubtitleChoice(null)}
                        class="w-full text-left px-3 py-2 rounded-lg text-xs text-gray-100 hover:bg-white/15 transition-colors"
                        class:bg-cyan-400={selectedSubtitleIndex === null}
                        class:bg-opacity-20={selectedSubtitleIndex === null}
                      >
                        Off
                      </button>

                      {#each mediaStreams.subtitle as track}
                        <button
                          type="button"
                          onclick={() => handleSubtitleChoice(track.index)}
                          class="w-full text-left px-3 py-2 rounded-lg text-xs text-gray-100 hover:bg-white/15 transition-colors"
                          class:bg-cyan-400={selectedSubtitleIndex === track.index}
                          class:bg-opacity-20={selectedSubtitleIndex === track.index}
                        >
                          {track.display_title}
                        </button>
                      {/each}
                    </div>
                  {/if}
                </div>
              {/if}

              <div class="relative">
                <button
                  onclick={() => toggleTopMenu("overflow")}
                  aria-label="More playback options"
                  class="h-8 w-8 grid place-items-center rounded-lg bg-black/50 border border-white/20 backdrop-blur-md text-white hover:bg-black/65 transition-colors"
                >
                  <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 5h.01M12 12h.01M12 19h.01" />
                  </svg>
                </button>

                {#if overflowMenuOpen}
                  <div class="absolute right-0 bottom-10 w-56 rounded-xl border border-white/20 bg-black/82 backdrop-blur-xl shadow-[0_18px_48px_rgba(0,0,0,0.55)] p-2 space-y-3">
                    <div>
                      <p class="px-2 pb-1 text-[11px] uppercase tracking-wide text-gray-400">Playback Speed</p>
                      <div class="grid grid-cols-3 gap-1">
                        {#each [0.75, 1, 1.25, 1.5, 1.75, 2] as speed}
                          <button
                            type="button"
                            onclick={() => setRate(speed)}
                            class="h-8 rounded-lg text-xs text-gray-100 border border-white/15 hover:bg-white/15 transition-colors"
                            class:bg-cyan-400={Math.abs(rate - speed) < 0.01}
                            class:bg-opacity-20={Math.abs(rate - speed) < 0.01}
                            class:border-cyan-300={Math.abs(rate - speed) < 0.01}
                          >
                            {speed}x
                          </button>
                        {/each}
                      </div>
                    </div>

                    <div>
                      <p class="px-2 pb-1 text-[11px] uppercase tracking-wide text-gray-400">Quality</p>
                      <div class="grid grid-cols-2 gap-1">
                        {#each qualityOptions as option}
                          <button
                            type="button"
                            onclick={() => applyQualitySelection(option.key)}
                            class="h-8 rounded-lg text-xs text-gray-100 border border-white/15 hover:bg-white/15 transition-colors"
                            class:bg-cyan-400={selectedQuality.key === option.key}
                            class:bg-opacity-20={selectedQuality.key === option.key}
                            class:border-cyan-300={selectedQuality.key === option.key}
                          >
                            {option.label}
                          </button>
                        {/each}
                      </div>
                    </div>
                  </div>
                {/if}
              </div>
            </div>

            {#if autoplayCountdown !== null}
              <span class="h-8 px-3 inline-flex items-center rounded-lg bg-amber-400/15 border border-amber-300/30 text-amber-200 text-xs font-medium">
                Next in {autoplayCountdown}s
              </span>
              <button
                onclick={cancelAutoplayCountdown}
                class="h-8 px-3 rounded-lg bg-white/10 border border-white/20 text-gray-100 text-xs font-medium hover:bg-white/20 transition-colors"
              >
                Cancel
              </button>
            {/if}
          </div>
        </div>

        <div
          bind:this={progressScrubEl}
          class="group w-full h-5 sm:h-6 flex items-center mb-2 cursor-grab"
          class:cursor-grabbing={isScrubbing}
          onpointerdown={beginTimelineScrub}
          onkeydown={handleProgressKeydown}
          role="slider"
          aria-label="Seek in video"
          aria-valuenow={Math.floor(effectivePos)}
          aria-valuemin={0}
          aria-valuemax={Math.floor(dur)}
          tabindex="0"
        >
          <div class="w-full h-2 bg-white/18 rounded-full transition-all relative overflow-visible group-hover:shadow-[0_0_0_1px_rgba(56,189,248,0.35)]">
            <div
              class="absolute top-0 left-0 h-full bg-gradient-to-r from-cyan-400 to-blue-500 rounded-full"
              style="width: {progressPercent}%"
            ></div>

            <div
              class="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 h-4 w-4 rounded-full bg-cyan-300 border border-cyan-100 shadow-[0_0_0_2px_rgba(0,0,0,0.35)] transition-transform"
              class:scale-125={isScrubbing}
              style="left: {progressPercent}%"
            ></div>

            {#each chapterMarkers as chapter}
              <button
                type="button"
                class="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-2 h-4 rounded bg-white/75 hover:bg-amber-300 transition-colors"
                style="left: {chapter.percent}%"
                onpointerdown={(e) => e.stopPropagation()}
                onclick={(e) => {
                  e.stopPropagation();
                  void seekToChapter(chapter.startSeconds);
                }}
                aria-label={`Jump to chapter: ${chapter.name}`}
                title={chapter.name}
              ></button>
            {/each}
          </div>
        </div>


        <div class="grid grid-cols-1 sm:grid-cols-[auto_1fr_auto] gap-2 items-center mb-1">
          <div class="text-sm text-gray-300">
            <span class="text-white font-medium tabular-nums">{formatTime(effectivePos)}</span>
            <span class="text-gray-400 ml-1">/ {formatTime(dur)}</span>
          </div>

          <div class="flex items-center justify-center gap-1.5 sm:gap-2">
            {#if previousEpisode}
              <button
                onclick={playPreviousEpisode}
                aria-label="Play previous episode"
                class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/15 text-white hover:bg-white/20 transition-colors"
              >
                <svg class="w-4.5 h-4.5" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M20.5 6v12l-8.3-6z" />
                  <path d="M12.5 6v12l-8.3-6z" />
                </svg>
              </button>
            {/if}

            <button
              onclick={seekBack10}
              aria-label="Seek back 10 seconds"
              class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/15 text-white hover:bg-white/20 transition-colors"
            >
              <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
                <path d="M11.92 5.08L4 12l7.92 6.92L10.5 20.5 1 12l9.5-8.5z" />
              </svg>
            </button>

            <button
              onclick={togglePause}
              aria-label={isPaused ? "Play" : "Pause"}
              class="h-11 w-11 sm:h-12 sm:w-12 grid place-items-center rounded-full bg-gradient-to-br from-cyan-300 to-blue-500 text-black shadow-[0_8px_24px_rgba(56,189,248,0.4)] hover:scale-[1.03] transition-transform"
            >
              {#if isPaused || playerStatus === "loading"}
                <svg class="w-7 h-7" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M8 5v14l11-7z" />
                </svg>
              {:else}
                <svg class="w-6 h-6" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M6 4h4v16H6zM14 4h4v16h-4z" />
                </svg>
              {/if}
            </button>

            <button
              onclick={seekForward30}
              aria-label="Seek forward 30 seconds"
              class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/15 text-white hover:bg-white/20 transition-colors"
            >
              <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12.08 5.08L20 12l-7.92 6.92L13.5 20.5 23 12l-9.5-8.5z" />
              </svg>
            </button>

            {#if nextEpisode}
              <button
                onclick={playNextEpisode}
                aria-label="Play next episode"
                class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/15 text-white hover:bg-white/20 transition-colors"
              >
                <svg class="w-4.5 h-4.5" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M3.5 6v12l8.3-6z" />
                  <path d="M11.5 6v12l8.3-6z" />
                </svg>
              </button>
            {/if}
          </div>

          <div class="flex items-center justify-end gap-1.5 text-sm text-gray-300">
            <button
              type="button"
              onclick={toggleMute}
              aria-label={muted ? "Unmute" : "Mute"}
              class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/15 hover:bg-white/20 transition-colors"
            >
              {#if muted}
                <svg class="w-4 h-4 text-gray-300" viewBox="0 0 20 20" fill="currentColor">
                  <path d="M9.383 3.076A1 1 0 0110 4v12a1 1 0 01-1.707.707L4.586 13H2a1 1 0 01-1-1V8a1 1 0 011-1h2.586l3.707-3.707a1 1 0 011.09-.217z"/>
                  <path d="M13.293 7.293a1 1 0 011.414 0L16 8.586l1.293-1.293a1 1 0 011.414 1.414L17.414 10l1.293 1.293a1 1 0 01-1.414 1.414L16 11.414l-1.293 1.293a1 1 0 01-1.414-1.414L14.586 10l-1.293-1.293a1 1 0 010-1.414z"/>
                </svg>
              {:else}
                <svg class="w-4 h-4 text-gray-300" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M9.383 3.076A1 1 0 0110 4v12a1 1 0 01-1.707.707L4.586 13H2a1 1 0 01-1-1V8a1 1 0 011-1h2.586l3.707-3.707a1 1 0 011.09-.217zM14.657 2.929a1 1 0 011.414 0A9.972 9.972 0 0119 10a9.972 9.972 0 01-2.929 7.071 1 1 0 01-1.414-1.414A7.971 7.971 0 0017 10c0-2.21-.894-4.208-2.343-5.657a1 1 0 010-1.414z" clip-rule="evenodd"/>
                </svg>
              {/if}
            </button>

            <input
              type="range"
              min="0"
              max="100"
              step="1"
              value={vol}
              oninput={handleVolumeInput}
              aria-label="Volume"
              class="w-24 h-1.5 accent-cyan-400 cursor-pointer"
            />
            <span class="text-xs w-7 text-right tabular-nums">{vol}</span>
          </div>
        </div>

      </div>
    </div>
  </div>
{/if}
