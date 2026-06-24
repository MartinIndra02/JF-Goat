<script lang="ts">
  import { push } from "svelte-spa-router";
  import type {
    MediaItem,
    Person,
    MediaStreamInfo,
    ExternalUrl,
    PlaybackSelection,
    OfflineDownload,
  } from "../../lib/types";
  import {
    setPreferredAudioStreamIndex,
    setPreferredSubtitleStreamIndex,
    setPreferredAudioMetadata,
    setPreferredSubtitleMetadata,
    resolvePreferredAudioStreamIndex,
    resolvePreferredSubtitleStreamIndex,
  } from "../../lib/stores/player.svelte";
  import {
    getDownloadStatus,
    startDownload,
    pauseDownload,
    resumeDownload,
    cancelDownload,
    deleteDownload,
  } from "../../lib/api";
  import { listen } from "@tauri-apps/api/event";
  import {
    seasonNumber, formatRuntime, formatDate,
    progressPercent, handleImageLoad, backdropUrl, personImageUrl, scrollCarousel,
    episodeThumbnailUrl,
  } from "./detailHelpers";

  let {
    item,
    siblingEpisodes = [],
    people = [],
    mediaStreams = null,
    externalUrls = [],
    onTogglePlayed,
    onToggleFavorite,
    onPlay,
  }: {
    item: MediaItem;
    siblingEpisodes: MediaItem[];
    people: Person[];
    mediaStreams: MediaStreamInfo | null;
    externalUrls: ExternalUrl[];
    onTogglePlayed: (id: string, played: boolean) => void;
    onToggleFavorite: (id: string, isFavorite: boolean) => void;
    onPlay: (item: MediaItem, fromStart?: boolean, selection?: PlaybackSelection) => void;
  } = $props();

  let overviewExpanded = $state(false);
  let contextMenuOpen = $state(false);
  let audioDropdownOpen = $state(false);
  let subtitleDropdownOpen = $state(false);
  let qualityDropdownOpen = $state(false);
  let selectedAudioIndex = $state<number | null>(null);
  let selectedSubtitleIndex = $state<number | null>(null);
  let selectedQualityKey = $state("original");
  let siblingScrollEl = $state<HTMLElement | null>(null);
  let castScrollEl = $state<HTMLElement | null>(null);

  let download = $state<OfflineDownload | null>(null);

  async function loadDownloadStatus() {
    try {
      download = await getDownloadStatus(item.id);
    } catch (e) {
      console.error("Failed to load download status:", e);
    }
  }

  $effect(() => {
    item.id;
    void loadDownloadStatus();
  });

  $effect(() => {
    let unlistenProgress: (() => void) | null = null;
    
    const setupListener = async () => {
      unlistenProgress = await listen<OfflineDownload>("download-progress", (event) => {
        const payload = event.payload;
        if (payload.id === item.id) {
          if (payload.status === "Deleted" || payload.status === "Cancelled") {
            download = null;
          } else {
            download = payload;
          }
        }
      });
    };
    
    void setupListener();
    
    return () => {
      if (unlistenProgress) unlistenProgress();
    };
  });

  async function handleDownloadClick() {
    if (!download) {
      try {
        await startDownload(item.id);
      } catch (e) {
        console.error("Failed to start download:", e);
      }
    } else {
      push("/offline");
    }
  }

  // Scroll to top and reset transient state when item changes
  $effect(() => {
    item.id; // track item identity
    window.scrollTo(0, 0);
    overviewExpanded = false;
    contextMenuOpen = false;
    audioDropdownOpen = false;
    subtitleDropdownOpen = false;
    qualityDropdownOpen = false;
    selectedAudioIndex = null;
    selectedSubtitleIndex = null;
    selectedQualityKey = "original";
  });

  // Set defaults from stream info
  $effect(() => {
    if (mediaStreams) {
      if (selectedAudioIndex === null) {
        selectedAudioIndex = resolvePreferredAudioStreamIndex(mediaStreams.audio);
      }

      if (selectedSubtitleIndex === null) {
        selectedSubtitleIndex = resolvePreferredSubtitleStreamIndex(mediaStreams.subtitle);
      }
    }
  });

  const selectedAudioLabel = $derived(() => {
    if (!mediaStreams) return null;
    const sel = mediaStreams.audio.find(a => a.index === selectedAudioIndex);
    return sel?.display_title ?? mediaStreams.audio[0]?.display_title ?? null;
  });

  const selectedSubtitleLabel = $derived(() => {
    if (!mediaStreams) return null;
    const sel = mediaStreams.subtitle.find(s => s.index === selectedSubtitleIndex);
    return sel?.display_title ?? null;
  });

  const qualityOptions = $derived.by(() => {
    const base = [{
      key: "original",
      label: "Original",
      maxStreamingBitrate: null,
      targetHeight: null,
    }];

    if (!mediaStreams || mediaStreams.video.length === 0) {
      return base;
    }

    const bitrateForHeight = (height: number): number => {
      if (height >= 2160) return 20_000_000;
      if (height >= 1080) return 8_000_000;
      if (height >= 720) return 4_500_000;
      if (height >= 480) return 2_000_000;
      return 1_000_000;
    };

    const heights = Array.from(
      new Set(
        mediaStreams.video
          .map((track) => track.height ?? null)
          .filter((height): height is number => !!height && height > 0),
      ),
    ).sort((a, b) => b - a);

    const transcodeProfiles = heights.map((height) => ({
      key: `h-${height}`,
      label: `${height}p`,
      maxStreamingBitrate: bitrateForHeight(height),
      targetHeight: height,
    }));

    return [...base, ...transcodeProfiles];
  });

  const selectedQuality = $derived(() => {
    const match = qualityOptions.find((option) => option.key === selectedQualityKey);
    return match ?? qualityOptions[0];
  });

  const selectedQualityLabel = $derived(() => selectedQuality().label);

  $effect(() => {
    const options = qualityOptions;
    if (!options.some((option) => option.key === selectedQualityKey)) {
      selectedQualityKey = options[0].key;
    }
  });

  function buildPlaybackSelection(): PlaybackSelection {
    const selectedAudioTrack = mediaStreams?.audio.find(
      (track) => track.index === selectedAudioIndex,
    );
    const selectedSubtitleTrack = mediaStreams?.subtitle.find(
      (track) => track.index === selectedSubtitleIndex,
    );

    return {
      audioStreamIndex: selectedAudioIndex,
      subtitleStreamIndex: selectedSubtitleIndex,
      audioLanguage: selectedAudioTrack?.language ?? null,
      subtitleLanguage: selectedSubtitleTrack?.language ?? null,
      audioDisplayTitle: selectedAudioTrack?.display_title ?? null,
      subtitleDisplayTitle: selectedSubtitleTrack?.display_title ?? null,
      maxStreamingBitrate: selectedQuality().maxStreamingBitrate,
      targetHeight: selectedQuality().targetHeight,
    };
  }

  function navigateToItem(id: string) { push(`/item?id=${id}`); }
  function goBack() { window.history.length > 1 ? window.history.back() : push("/home"); }
  function closeContextMenu() { contextMenuOpen = false; }
  function closeDropdowns() {
    audioDropdownOpen = false;
    subtitleDropdownOpen = false;
    qualityDropdownOpen = false;
  }
</script>

<main class="app-stage min-h-screen text-[var(--text-primary)]">
  <!-- Hero Backdrop -->
  <div class="relative w-full overflow-hidden" style="height: clamp(340px, 55vh, 560px);">
    {#if backdropUrl(item)}
      <img
        src={backdropUrl(item)}
        alt=""
        onload={handleImageLoad}
        class="absolute inset-0 w-full h-full object-cover object-top transition-opacity duration-500 opacity-0"
      />
    {/if}
    <div class="absolute inset-0 bg-[rgba(5,7,13,0.5)] -z-10"></div>
    <div class="absolute inset-0 bg-gradient-to-t from-[var(--bg-0)] via-[rgba(5,7,13,0.4)] to-transparent"></div>

    <!-- Back button -->
    <button
      aria-label="Go back"
      onclick={goBack}
      class="absolute top-4 left-4 z-10 h-10 w-10 grid place-items-center bg-[rgba(10,18,31,0.64)] border border-white/22 rounded-xl backdrop-blur-xl text-[var(--text-primary)] hover:bg-[rgba(22,34,54,0.76)] transition-colors"
    >
      <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
      </svg>
    </button>

  </div>

  <div class="relative -mt-32 z-10 px-5 pb-16 max-w-3xl mx-auto">
    <!-- Header -->
    <div class="text-center mb-6">
      {#if item.type === "Episode" && item.series_name}
        <button
          onclick={() => item?.series_id && navigateToItem(item.series_id)}
          class="text-blue-400 hover:text-blue-300 text-lg font-bold mb-1 transition-colors cursor-pointer inline-flex items-center gap-1"
        >
          <span>{item.series_name}</span>
        </button>
      {/if}

      <!-- Title: S6 - E2 - Home -->
      <h1 class="text-2xl sm:text-3xl font-bold text-white leading-tight mb-1">
        {#if item.type === "Episode" && item.season_name && item.index_number}
          S{seasonNumber(item.season_name)} - E{item.index_number} - {item.name}
        {:else}
          {item.name}
        {/if}
      </h1>

      <!-- Metadata row -->
      <div class="flex flex-wrap items-center justify-center gap-2 mb-3">
        {#if item.official_rating}
          <span class="inline-flex items-center gap-1 text-xs font-semibold text-gray-200 bg-white/10 px-2.5 py-1 rounded-md border border-white/15">{item.official_rating}</span>
          <span class="w-1 h-1 rounded-full bg-gray-500"></span>
        {/if}
        {#if item.date_created && item.type === "Episode"}
          <span class="inline-flex items-center gap-1.5 text-xs text-gray-300 bg-white/8 px-2.5 py-1 rounded-md">
            <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clip-rule="evenodd"/></svg>
            {formatDate(item.date_created)}
          </span>
        {:else if item.production_year}
          <span class="inline-flex items-center gap-1.5 text-xs text-gray-300 bg-white/8 px-2.5 py-1 rounded-md">
            <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clip-rule="evenodd"/></svg>
            {item.production_year}
          </span>
        {/if}
        {#if item.run_time_ticks}
          <span class="w-1 h-1 rounded-full bg-gray-500"></span>
          <span class="inline-flex items-center gap-1.5 text-xs text-gray-300 bg-white/8 px-2.5 py-1 rounded-md">
            <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd"/></svg>
            {formatRuntime(item.run_time_ticks)}
          </span>
        {/if}
        {#if item.community_rating}
          <span class="w-1 h-1 rounded-full bg-gray-500"></span>
          <span class="inline-flex items-center gap-1.5 text-xs font-medium bg-amber-500/20 text-amber-300 px-2.5 py-1 rounded-md">
            <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"/></svg>
            {item.community_rating.toFixed(1)}
          </span>
        {/if}
      </div>

      <!-- Genre badges -->
      {#if item.genres}
        <div class="flex flex-wrap justify-center gap-1.5 mb-4">
          {#each item.genres.split(",").slice(0, 6) as genre}
            <span class="text-xs text-gray-300 bg-white/10 px-2.5 py-1 rounded-full border border-white/5">{genre.trim()}</span>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Play / Resume button -->
    <div class="mb-2">
      <button onclick={() => onPlay(item, false, buildPlaybackSelection())} class="relative w-full flex items-center justify-center gap-2.5 py-3.5 bg-gradient-to-br from-cyan-200 via-sky-300 to-amber-300 text-slate-950 hover:brightness-110 rounded-xl font-bold text-sm shadow-[0_10px_26px_rgba(102,216,255,0.25)] hover:scale-[1.01] active:scale-[0.99] transition-all overflow-hidden">
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
        {#if progressPercent(item) > 0}
          <span>Resume</span>
          <div class="absolute bottom-0 left-0 right-0 h-1 bg-slate-900/20">
            <div class="h-full bg-slate-950/60 transition-all" style="width: {progressPercent(item)}%"></div>
          </div>
        {:else}
          <span>Play {item.type === "Episode" ? `S${seasonNumber(item.season_name)} - E${item.index_number}` : item.name}</span>
        {/if}
      </button>
    </div>

    <!-- Play from start (only if there's progress) -->
    {#if progressPercent(item) > 0}
      <div class="mb-3">
        <button onclick={() => onPlay(item, true, buildPlaybackSelection())} class="w-full flex items-center justify-center gap-2 py-2.5 bg-white/8 hover:bg-white/14 border border-white/10 rounded-xl text-sm font-medium text-[var(--text-secondary)] hover:text-white transition-colors">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg>
          Play from start
        </button>
      </div>
    {/if}

    <!-- Media spec badges -->
    {#if mediaStreams}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="flex flex-wrap items-center gap-2 mb-3 justify-center">
        <!-- Audio dropdown -->
        {#if mediaStreams.audio.length > 0}
          <div class="relative">
            <button
              onclick={() => { subtitleDropdownOpen = false; audioDropdownOpen = !audioDropdownOpen; }}
              class="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--text-primary)] bg-[rgba(10,18,31,0.64)] border border-white/22 px-3 py-2 rounded-xl hover:bg-[rgba(22,34,54,0.76)] transition-colors cursor-pointer"
            >
              <svg class="w-3.5 h-3.5 text-cyan-200" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M9.383 3.076A1 1 0 0110 4v12a1 1 0 01-1.707.707L4.586 13H2a1 1 0 01-1-1V8a1 1 0 011-1h2.586l3.707-3.707a1 1 0 011.09-.217zM14.657 2.929a1 1 0 011.414 0A9.972 9.972 0 0119 10a9.972 9.972 0 01-2.929 7.071a1 1 0 01-1.414-1.414A7.971 7.971 0 0017 10c0-2.21-.894-4.208-2.343-5.657a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
              {selectedAudioLabel()}
              <svg class="w-3 h-3 text-cyan-200" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
            </button>
            {#if audioDropdownOpen}
              <div onclick={() => audioDropdownOpen = false} class="fixed inset-0 z-40"></div>
              <div class="absolute left-1/2 -translate-x-1/2 top-full mt-1.5 w-64 bg-[rgba(7,14,24,0.92)] border border-white/22 rounded-xl shadow-xl z-50 p-1.5 max-h-64 overflow-y-auto backdrop-blur-2xl">
                {#each mediaStreams.audio as track}
                  <button
                    onclick={() => {
                      selectedAudioIndex = track.index;
                      setPreferredAudioStreamIndex(track.index);
                      setPreferredAudioMetadata(track.language, track.display_title);
                      audioDropdownOpen = false;
                    }}
                    class="w-full text-left px-3 py-2 rounded-lg text-xs transition-colors text-gray-100 hover:bg-white/14 flex items-center gap-2 {track.index === selectedAudioIndex ? 'bg-cyan-400 bg-opacity-20 text-cyan-200' : ''}"
                  >
                    {track.display_title}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}

        <!-- Subtitle dropdown -->
        {#if mediaStreams.subtitle.length > 0}
          <div class="relative">
            <button
              onclick={() => { audioDropdownOpen = false; subtitleDropdownOpen = !subtitleDropdownOpen; }}
              class="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--text-primary)] bg-[rgba(10,18,31,0.64)] border border-white/22 px-3 py-2 rounded-xl hover:bg-[rgba(22,34,54,0.76)] transition-colors cursor-pointer"
            >
              <svg class="w-3.5 h-3.5 text-emerald-200" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M4 3a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V5a2 2 0 00-2-2H4zm2 3a1 1 0 000 2h3a1 1 0 000-2H6zm0 4a1 1 0 000 2h8a1 1 0 100-2H6zm0 4a1 1 0 100 2h5a1 1 0 100-2H6z" clip-rule="evenodd"/></svg>
              {selectedSubtitleLabel() ?? "Subtitles"}
              <svg class="w-3 h-3 text-emerald-200" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
            </button>
            {#if subtitleDropdownOpen}
              <div onclick={() => subtitleDropdownOpen = false} class="fixed inset-0 z-40"></div>
              <div class="absolute left-1/2 -translate-x-1/2 top-full mt-1.5 w-64 bg-[rgba(7,14,24,0.92)] border border-white/22 rounded-xl shadow-xl z-50 p-1.5 max-h-64 overflow-y-auto backdrop-blur-2xl">
                <button
                  onclick={() => {
                    selectedSubtitleIndex = null;
                    setPreferredSubtitleStreamIndex(null);
                    setPreferredSubtitleMetadata(null, null);
                    subtitleDropdownOpen = false;
                  }}
                  class="w-full text-left px-3 py-2 rounded-lg text-xs transition-colors text-gray-100 hover:bg-white/14 {selectedSubtitleIndex === null ? 'bg-cyan-400 bg-opacity-20 text-cyan-200' : ''}"
                >None</button>
                {#each mediaStreams.subtitle as track}
                  <button
                    onclick={() => {
                      selectedSubtitleIndex = track.index;
                      setPreferredSubtitleStreamIndex(track.index);
                      setPreferredSubtitleMetadata(track.language, track.display_title);
                      subtitleDropdownOpen = false;
                    }}
                    class="w-full text-left px-3 py-2 rounded-lg text-xs transition-colors text-gray-100 hover:bg-white/14 flex items-center gap-2 {track.index === selectedSubtitleIndex ? 'bg-cyan-400 bg-opacity-20 text-cyan-200' : ''}"
                  >
                    {track.display_title}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    <!-- Separator -->
    <div class="flex justify-center my-2">
      <div class="w-3 h-0.5 rounded-full bg-white/20"></div>
    </div>

    <!-- Action buttons -->
    <div class="flex items-center justify-center gap-2 mb-5">
      <button aria-label="Toggle favorite" onclick={() => onToggleFavorite(item.id, item.is_favorite)} class="p-2.5 rounded-xl bg-white/5 hover:bg-white/12 border border-white/10 transition-all {item.is_favorite ? 'text-rose-400 hover:text-rose-300 bg-rose-500/10 border-rose-500/20' : 'text-gray-400 hover:text-white'}">
        <svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/></svg>
      </button>
      <button aria-label="Toggle watched" onclick={() => onTogglePlayed(item.id, item.played)} class="p-2.5 rounded-xl bg-white/5 hover:bg-white/12 border border-white/10 transition-all {item.played ? 'text-emerald-400 hover:text-emerald-300 bg-emerald-500/10 border-emerald-500/20' : 'text-gray-400 hover:text-white'}">
        <svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg>
      </button>

      <!-- Download Button -->
      <button
        onclick={handleDownloadClick}
        class="p-2.5 rounded-xl border transition-all cursor-pointer flex items-center justify-center gap-1.5 {download ? (download.status === 'Completed' ? 'text-emerald-400 bg-emerald-500/10 border-emerald-500/20' : download.status === 'Downloading' ? 'text-cyan-400 bg-cyan-500/10 border-cyan-500/20 px-3' : download.status === 'Pending' ? 'text-amber-400 bg-amber-500/10 border-amber-500/20 px-3' : download.status === 'Failed' ? 'text-rose-400 bg-rose-500/10 border-rose-500/20' : 'text-gray-400 bg-white/5 border-white/10 hover:bg-white/12 hover:text-white') : 'text-gray-400 bg-white/5 border-white/10 hover:bg-white/12 hover:text-white'}"
        aria-label="Download"
        title={download ? `Offline download: ${download.status}` : "Download offline"}
      >
        {#if !download}
          <svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
        {:else if download.status === "Downloading"}
          <svg class="w-5 h-5 text-cyan-300 animate-spin" fill="none" viewBox="0 0 24 24">
            <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25" />
            <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
          </svg>
          <span class="text-xs font-semibold text-cyan-200">{download.progress.toFixed(0)}%</span>
        {:else if download.status === "Pending"}
          <svg class="w-5 h-5 text-amber-300 animate-pulse" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd"/></svg>
          <span class="text-xs font-semibold text-amber-200">Queued</span>
        {:else if download.status === "Paused"}
          <svg class="w-5 h-5 text-gray-300" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zM7 8a1 1 0 012 0v4a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v4a1 1 0 102 0V8a1 1 0 00-1-1z" clip-rule="evenodd"/></svg>
          <span class="text-xs font-semibold text-gray-300">Paused</span>
        {:else if download.status === "Completed"}
          <svg class="w-5 h-5 text-emerald-400" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/></svg>
        {:else if download.status === "Failed"}
          <svg class="w-5 h-5 text-rose-400" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z" clip-rule="evenodd"/></svg>
          <span class="text-xs font-semibold text-rose-300">Failed</span>
        {/if}
      </button>

      <!-- Context menu -->
      <div class="relative">
        <button aria-label="More options" onclick={() => contextMenuOpen = !contextMenuOpen} class="p-2.5 rounded-xl bg-white/5 hover:bg-white/12 border border-white/10 text-gray-400 hover:text-white transition-colors">
          <svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path d="M6 10a2 2 0 11-4 0 2 2 0 014 0zM12 10a2 2 0 11-4 0 2 2 0 014 0zM16 12a2 2 0 100-4 2 2 0 000 4z"/></svg>
        </button>
        {#if contextMenuOpen}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div onclick={closeContextMenu} class="fixed inset-0 z-40"></div>
          <div class="absolute right-0 top-full mt-1.5 w-52 bg-[rgba(7,14,24,0.92)] border border-white/22 rounded-xl shadow-xl z-50 py-1.5 backdrop-blur-2xl overflow-hidden">
            {#if item.type === "Episode" && item.series_id}
              <button onclick={() => { closeContextMenu(); item?.series_id && navigateToItem(item.series_id); }} class="w-full text-left px-4 py-2.5 text-sm text-gray-200 hover:bg-white/10 transition-colors flex items-center gap-2.5">
                <svg class="w-4 h-4 text-gray-400" viewBox="0 0 20 20" fill="currentColor"><path d="M7 3a1 1 0 000 2h6a1 1 0 100-2H7zM4 7a1 1 0 011-1h10a1 1 0 110 2H5a1 1 0 01-1-1zM2 11a2 2 0 012-2h12a2 2 0 012 2v4a2 2 0 01-2 2H4a2 2 0 01-2-2v-4z"/></svg>
                Open show
              </button>
            {/if}
            <button onclick={() => { onPlay(item, true, buildPlaybackSelection()); closeContextMenu(); }} class="w-full text-left px-4 py-2.5 text-sm text-gray-200 hover:bg-white/10 transition-colors flex items-center gap-2.5">
              <svg class="w-4 h-4 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg>
              Play from start
            </button>
            <button onclick={closeContextMenu} class="w-full text-left px-4 py-2.5 text-sm text-gray-200 hover:bg-white/10 transition-colors flex items-center gap-2.5">
              <svg class="w-4 h-4 text-gray-400" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-11a1 1 0 10-2 0v2H7a1 1 0 100 2h2v2a1 1 0 102 0v-2h2a1 1 0 100-2h-2V7z" clip-rule="evenodd"/></svg>
              Add to playlist
            </button>
            <button onclick={() => { onTogglePlayed(item.id, item.played); closeContextMenu(); }} class="w-full text-left px-4 py-2.5 text-sm text-gray-200 hover:bg-white/10 transition-colors flex items-center gap-2.5">
              <svg class="w-4 h-4 text-gray-400" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg>
              {item.played ? "Mark as unwatched" : "Mark as watched"}
            </button>
            <button onclick={() => { onToggleFavorite(item.id, item.is_favorite); closeContextMenu(); }} class="w-full text-left px-4 py-2.5 text-sm text-gray-200 hover:bg-white/10 transition-colors flex items-center gap-2.5">
              <svg class="w-4 h-4 text-gray-400" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/></svg>
              {item.is_favorite ? "Remove from favorites" : "Add to favorites"}
            </button>
          </div>
        {/if}
      </div>
    </div>

    <!-- Overview -->
    {#if item.overview}
      <div class="mb-6">
        <h2 class="text-base font-semibold text-white mb-2">Overview</h2>
        <p class="text-gray-300 text-sm leading-relaxed {overviewExpanded ? '' : 'line-clamp-3'}">{item.overview}</p>
        {#if item.overview.length > 200}
          <button onclick={() => overviewExpanded = !overviewExpanded} class="text-blue-400 hover:text-blue-300 text-xs font-medium mt-1.5 transition-colors flex items-center gap-1">
            {overviewExpanded ? "Show less" : "Show more"}
            <svg class="w-3 h-3 transition-transform {overviewExpanded ? 'rotate-180' : ''}" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
          </button>
        {/if}
      </div>
    {/if}

    <!-- Cast & Crew -->
    {#if people.length > 0}
      <div class="mb-8">
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-base font-semibold text-white">Cast & Crew</h2>
          <div class="flex items-center gap-1">
            <button aria-label="Scroll cast left" onclick={() => scrollCarousel(castScrollEl, 'left')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></button>
            <button aria-label="Scroll cast right" onclick={() => scrollCarousel(castScrollEl, 'right')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd"/></svg></button>
          </div>
        </div>
        <div bind:this={castScrollEl} class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-5 px-5">
          {#each people as person, i (person.id + '-' + i)}
            <div class="flex-shrink-0 w-[80px] text-center">
              <div class="relative w-[72px] h-[72px] mx-auto rounded-lg overflow-hidden bg-gray-800 mb-1.5 ring-1 ring-white/10">
                {#if person.image_tag}
                  <img src={personImageUrl(person.id, person.image_tag)} alt={person.name} onload={handleImageLoad} class="w-full h-full object-cover transition-opacity duration-300 opacity-0" />
                {:else}
                  <div class="w-full h-full flex items-center justify-center">
                    <svg class="w-7 h-7 text-gray-600" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clip-rule="evenodd"/></svg>
                  </div>
                {/if}
              </div>
              <p class="text-[11px] text-white font-medium truncate leading-tight">{person.name}</p>
              {#if person.role}
                <p class="text-[10px] text-gray-500 truncate leading-tight">{person.role}</p>
              {:else if person.person_type}
                <p class="text-[10px] text-gray-500 truncate leading-tight">{person.person_type}</p>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <!-- More from season X -->
    {#if item.type === "Episode" && siblingEpisodes.length > 1}
      <div class="mb-8">
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-base font-semibold text-white">More from {item.season_name ?? "this season"}</h2>
          <div class="flex items-center gap-1">
            <button aria-label="Scroll episodes left" onclick={() => scrollCarousel(siblingScrollEl, 'left')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></button>
            <button aria-label="Scroll episodes right" onclick={() => scrollCarousel(siblingScrollEl, 'right')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd"/></svg></button>
          </div>
        </div>
        <div bind:this={siblingScrollEl} class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-5 px-5">
          {#each siblingEpisodes as episode (episode.id)}
            <button
              onclick={() => navigateToItem(episode.id)}
              class="flex-shrink-0 w-48 rounded-xl overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer {episode.id === item?.id ? 'ring-2 ring-cyan-400/80' : ''}"
            >
              <div class="relative">
                {#if episodeThumbnailUrl(episode)}
                  <img src={episodeThumbnailUrl(episode)} alt={episode.name} onload={handleImageLoad} class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0" />
                {:else}
                  <div class="w-full aspect-video bg-gray-800 flex items-center justify-center"><span class="text-gray-500 text-xs">E{episode.index_number ?? "?"}</span></div>
                {/if}
                <div class="absolute inset-0 bg-gray-800 -z-10"></div>
                {#if episode.played}
                  <div class="absolute top-1.5 right-1.5 bg-green-500/90 rounded-full p-0.5"><svg class="w-3 h-3 text-white" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></div>
                {/if}
                {#if progressPercent(episode) > 0}
                  <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50"><div class="h-full bg-blue-500 rounded-r-full" style="width: {progressPercent(episode)}%"></div></div>
                {/if}
              </div>
              <div class="p-2.5">
                <p class="text-[11px] text-gray-500 mb-0.5">
                  S{seasonNumber(episode.season_name)} - E{episode.index_number ?? "?"}
                  {#if episode.run_time_ticks}<span class="ml-1">· {formatRuntime(episode.run_time_ticks)}</span>{/if}
                </p>
                <p class="text-sm text-white truncate font-medium">{episode.name}</p>
              </div>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <!-- External Links -->
    {#if externalUrls.length > 0}
      <div class="mb-8">
        <h2 class="text-base font-semibold text-white mb-3">External</h2>
        <div class="flex flex-wrap gap-2">
          {#each externalUrls as extUrl}
            <a href={extUrl.url} target="_blank" rel="noopener noreferrer" class="inline-flex items-center gap-1.5 text-xs text-blue-400 hover:text-blue-300 bg-white/5 hover:bg-white/10 px-3 py-1.5 rounded-lg border border-white/10 transition-colors">
              <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path d="M11 3a1 1 0 100 2h2.586l-6.293 6.293a1 1 0 101.414 1.414L15 6.414V9a1 1 0 102 0V4a1 1 0 00-1-1h-5z"/><path d="M5 5a2 2 0 00-2 2v8a2 2 0 002 2h8a2 2 0 002-2v-3a1 1 0 10-2 0v3H5V7h3a1 1 0 000-2H5z"/></svg>
              {extUrl.name}
            </a>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</main>
