<script lang="ts">
  import { onMount } from "svelte";
  import {
    listen,
    type Event as TauriEvent,
    type UnlistenFn,
  } from "@tauri-apps/api/event";
  import {
    getSeasonEpisodes,
    getSeriesSeasons,
    mpvPlay,
    togglePlayed,
  } from "../../lib/api";
  import {
    getPreferredAudioStreamIndex,
    getPreferredSubtitleStreamIndex,
    showPlayer,
  } from "../../lib/stores/player.svelte";
  import {
    IMAGE_CACHED_EVENT,
    imageCacheKey,
    imageCacheKeyFromUrl,
    withCacheBust,
    type ImageCachedPayload,
  } from "../../views/detail/detailHelpers";
  import { pushErrorToast, pushToast } from "../../lib/stores/toast.svelte";
  import { push } from "svelte-spa-router";
  import type { MediaItem } from "../../lib/types";

  let {
    item,
    landscape = false,
  }: {
    item: MediaItem;
    landscape?: boolean;
  } = $props();

  function formatRuntime(ticks: number | null): string {
    if (!ticks) return "";
    const minutes = Math.round(ticks / 600_000_000);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  }

  function progressPercent(item: MediaItem): number {
    if (!item.run_time_ticks || !item.playback_ticks || item.playback_ticks <= 0) return 0;
    return Math.min((item.playback_ticks / item.run_time_ticks) * 100, 100);
  }

  function hashString(value: string): number {
    let hash = 0;
    for (let i = 0; i < value.length; i += 1) {
      hash = ((hash << 5) - hash + value.charCodeAt(i)) | 0;
    }
    return Math.abs(hash);
  }

  function buildPosterGradient(target: MediaItem): string {
    const seed = hashString(`${target.id}:${target.name}`);
    const hueA = seed % 360;
    const hueB = (hueA + 46 + (seed % 70)) % 360;
    return `linear-gradient(145deg, hsl(${hueA} 78% 58%), hsl(${hueB} 62% 34%))`;
  }

  const progress = $derived(progressPercent(item));
  const aspectClass = $derived(landscape ? "aspect-video" : "aspect-[2/3]");
  const baseImageSrc = $derived(getCardImageSrc(item, landscape));
  const trackedImageKey = $derived(imageCacheKeyFromUrl(baseImageSrc));
  let imageRefreshNonce = $state(0);
  const renderedImageSrc = $derived(withCacheBust(baseImageSrc, imageRefreshNonce));
  const posterFallbackStyle = $derived(`--poster-fallback-gradient: ${buildPosterGradient(item)};`);
  let launchingPlayback = $state(false);
  let imageLoaded = $state(false);
  let contextMenuOpen = $state(false);
  let contextMenuX = $state(0);
  let contextMenuY = $state(0);

  const canTogglePlayed = $derived(item.type === "Movie" || item.type === "Episode");
  const canGoToShow = $derived(item.type === "Episode" && !!item.series_id);
  const canGoToSeason = $derived(item.type === "Episode" && !!item.season_id);
  const canGoToEpisode = $derived(item.type === "Series" || item.type === "Season");

  function openDetail() {
    closeContextMenu();
    push(`/item?id=${item.id}`);
  }

  function closeContextMenu() {
    contextMenuOpen = false;
  }

  function handleContextMenu(event: MouseEvent) {
    event.preventDefault();

    const menuWidth = 210;
    const menuHeight = 230;
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    contextMenuX = Math.min(event.clientX, viewportWidth - menuWidth - 12);
    contextMenuY = Math.min(event.clientY, viewportHeight - menuHeight - 12);
    contextMenuOpen = true;
  }

  function handleMenuKeyboard(event: KeyboardEvent) {
    if (!contextMenuOpen) return;
    if (event.key === "Escape") {
      closeContextMenu();
    }
  }

  async function handleContextTogglePlayed() {
    if (!canTogglePlayed) return;

    try {
      const newPlayed = await togglePlayed(item.id, item.played);
      item = {
        ...item,
        played: newPlayed,
        playback_ticks: newPlayed ? item.playback_ticks : 0,
      };
      pushToast({
        level: "success",
        source: "api",
        title: newPlayed ? "Marked as watched" : "Marked as unwatched",
        message: item.name,
        dedupeKey: `poster-toggle-played-${item.id}-${newPlayed}`,
        dismissAfterMs: 2200,
      });
    } catch (error) {
      pushErrorToast(
        "api",
        error,
        "Could not update watched state",
        `poster-toggle-played-failed-${item.id}`,
      );
    } finally {
      closeContextMenu();
    }
  }

  function handleContextGoToShow() {
    if (!item.series_id) return;
    closeContextMenu();
    push(`/item?id=${item.series_id}`);
  }

  function handleContextGoToSeason() {
    if (!item.season_id) return;
    closeContextMenu();
    push(`/item?id=${item.season_id}`);
  }

  async function handleContextGoToEpisode() {
    if (!canGoToEpisode) return;
    const target = await resolvePlayableItem(item);
    if (!target) {
      closeContextMenu();
      return;
    }

    closeContextMenu();
    push(`/item?id=${target.id}`);
  }

  function seasonNumber(value: string | null): number | string {
    if (!value) return "?";
    const m = value.match(/\d+/);
    return m ? Number(m[0]) : "?";
  }

  function playbackTitle(target: MediaItem): string {
    if (target.type === "Episode" && target.series_name) {
      const sNum = seasonNumber(target.season_name);
      return `${target.series_name} - S${sNum} E${target.index_number ?? "?"} - ${target.name}`;
    }
    return target.name;
  }

  function pickEpisode(episodes: MediaItem[]): MediaItem | null {
    if (episodes.length === 0) return null;

    const inProgress = episodes.find((ep) => ep.playback_ticks > 0 && !ep.played);
    if (inProgress) return inProgress;

    const firstUnplayed = episodes.find((ep) => !ep.played);
    return firstUnplayed ?? episodes[0];
  }

  async function resolvePlayableItem(target: MediaItem): Promise<MediaItem | null> {
    if (target.type === "Movie" || target.type === "Episode") {
      return target;
    }

    if (target.type === "Season") {
      const seasonEpisodes = await getSeasonEpisodes(target.id).catch(() => []);
      return pickEpisode(seasonEpisodes);
    }

    if (target.type === "Series") {
      const seasons = await getSeriesSeasons(target.id).catch(() => []);
      if (seasons.length === 0) return null;

      const episodeLists = await Promise.all(
        seasons.map((season) => getSeasonEpisodes(season.id).catch(() => [])),
      );
      return pickEpisode(episodeLists.flat());
    }

    return null;
  }

  async function handlePosterClick() {
    if (launchingPlayback) return;

    closeContextMenu();

    launchingPlayback = true;
    try {
      const playableItem = await resolvePlayableItem(item);
      if (!playableItem) {
        openDetail();
        return;
      }

      const startTicks = playableItem.playback_ticks;
      showPlayer(playableItem.id, playbackTitle(playableItem));

      const preferredAudio = getPreferredAudioStreamIndex();
      const preferredSubtitle = getPreferredSubtitleStreamIndex();

      await mpvPlay({
        itemId: playableItem.id,
        startTicks,
        audioStreamIndex: preferredAudio ?? null,
        subtitleStreamIndex:
          preferredSubtitle === undefined
            ? null
            : preferredSubtitle === null
              ? -1
              : preferredSubtitle,
        maxStreamingBitrate: null,
        targetHeight: null,
      });
    } catch (e) {
      console.error("Failed to start playback from poster:", e);
      openDetail();
    } finally {
      launchingPlayback = false;
    }
  }

  function getCardImageSrc(target: MediaItem, landscapeMode: boolean): string {
    if (landscapeMode && target.backdrop_tag) {
      return `http://jfimage.localhost/backdrop/${target.id}?tag=${target.backdrop_tag}`;
    }
    if (target.image_tag) {
      return `http://jfimage.localhost/poster/${target.id}?tag=${target.image_tag}`;
    }
    if (target.series_id) {
      return `http://jfimage.localhost/poster/${target.series_id}?tag=${target.series_id}`;
    }
    return "";
  }

  function onImageCached(event: TauriEvent<ImageCachedPayload>) {
    if (!trackedImageKey) return;

    const payload = event.payload;
    const cachedKey = imageCacheKey(payload.image_type, payload.item_id, payload.tag);
    if (cachedKey !== trackedImageKey) return;

    imageRefreshNonce = Date.now();
  }

  onMount(() => {
    let disposed = false;
    let unlisten: UnlistenFn | null = null;

    listen<ImageCachedPayload>(IMAGE_CACHED_EVENT, onImageCached)
      .then((fn) => {
        if (disposed) {
          fn();
          return;
        }
        unlisten = fn;
      })
      .catch(() => {
        // Non-tauri contexts can safely ignore event wiring.
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  $effect(() => {
    renderedImageSrc;
    imageLoaded = false;
  });

  // Retry loading images that were returned as transparent placeholders (cache miss).
  // The background fetch will populate the cache, so the retry will succeed.
  function handleImageLoad(event: Event) {
    const img = event.target as HTMLImageElement;
    if (img.naturalWidth <= 1 && img.naturalHeight <= 1) {
      imageLoaded = false;
      // Got the transparent placeholder — image is being fetched in background
      const src = img.src;
      const retryCount = parseInt(img.dataset.retry ?? "0");
      if (retryCount < 3) {
        setTimeout(() => {
          img.dataset.retry = String(retryCount + 1);
          // Force reload by busting the cached response
          img.src = "";
          img.src = src;
        }, 1500 * (retryCount + 1));
      }
    } else {
      imageLoaded = true;
    }
  }

  function handleImageError() {
    imageLoaded = false;
  }
</script>

<svelte:window onkeydown={handleMenuKeyboard} />

<div
  class="group flex-shrink-0 {landscape ? 'w-56 sm:w-64' : 'w-32 sm:w-36'}"
>
  <button
    type="button"
    onclick={handlePosterClick}
    oncontextmenu={handleContextMenu}
    disabled={launchingPlayback}
    aria-busy={launchingPlayback}
    class="block w-full text-left relative overflow-hidden rounded-lg shadow-md transition-transform duration-200 group-hover:scale-105 group-hover:shadow-xl cursor-pointer focus-visible:outline focus-visible:outline-2 focus-visible:outline-blue-400 disabled:opacity-70"
    aria-label="Play {item.name}"
  >
    <div class="relative w-full {aspectClass}">
      <div class="poster-fallback">
        <div class="poster-fallback__surface" style={posterFallbackStyle}>
        </div>
      </div>

      {#if baseImageSrc}
        <img
          src={renderedImageSrc}
          alt={item.name}
          loading="lazy"
          onload={handleImageLoad}
          onerror={handleImageError}
          class="absolute inset-0 w-full h-full object-cover transition-opacity duration-300 {imageLoaded ? 'opacity-100' : 'opacity-0'}"
        />
      {/if}
    </div>

    <!-- Background placeholder behind the image -->
    <div class="absolute inset-0 bg-gray-800 -z-10"></div>

    <div class="absolute inset-0 bg-black/0 group-hover:bg-black/30 transition-colors duration-200"></div>

    {#if progress > 0}
      <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
        <div
          class="h-full bg-blue-500 rounded-r-full"
          style="width: {progress}%"
        ></div>
      </div>
    {/if}

    {#if item.type === "Episode" && item.series_name}
      <div class="absolute top-1.5 left-1.5">
        <span class="bg-black/70 text-white text-[10px] px-1.5 py-0.5 rounded-sm backdrop-blur-sm">
          Ep {item.index_number ?? "?"}
        </span>
      </div>
    {/if}
  </button>

  {#if contextMenuOpen}
    <button
      type="button"
      class="fixed inset-0 z-50"
      aria-label="Close item menu"
      onclick={closeContextMenu}
    ></button>

    <div
      class="fixed z-[60] w-52 overflow-hidden rounded-xl border border-white/10 bg-gray-900/96 shadow-2xl backdrop-blur-md"
      style="left: {contextMenuX}px; top: {contextMenuY}px;"
      role="menu"
      aria-label="Item actions"
    >
      <button
        type="button"
        role="menuitem"
        onclick={openDetail}
        class="w-full px-3 py-2.5 text-left text-sm text-gray-100 hover:bg-white/10 transition-colors"
      >
        Open details
      </button>

      {#if canTogglePlayed}
        <button
          type="button"
          role="menuitem"
          onclick={handleContextTogglePlayed}
          class="w-full px-3 py-2.5 text-left text-sm text-gray-100 hover:bg-white/10 transition-colors"
        >
          {item.played ? "Mark as unwatched" : "Mark as watched"}
        </button>
      {/if}

      {#if canGoToShow}
        <button
          type="button"
          role="menuitem"
          onclick={handleContextGoToShow}
          class="w-full px-3 py-2.5 text-left text-sm text-gray-100 hover:bg-white/10 transition-colors"
        >
          Go to show
        </button>
      {/if}

      {#if canGoToSeason}
        <button
          type="button"
          role="menuitem"
          onclick={handleContextGoToSeason}
          class="w-full px-3 py-2.5 text-left text-sm text-gray-100 hover:bg-white/10 transition-colors"
        >
          Go to season
        </button>
      {/if}

      {#if canGoToEpisode}
        <button
          type="button"
          role="menuitem"
          onclick={handleContextGoToEpisode}
          class="w-full px-3 py-2.5 text-left text-sm text-gray-100 hover:bg-white/10 transition-colors"
        >
          Go to episode
        </button>
      {/if}
    </div>
  {/if}

  <button
    type="button"
    onclick={openDetail}
    oncontextmenu={handleContextMenu}
    class="mt-1.5 px-0.5 block w-full text-left cursor-pointer rounded-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-blue-400"
    aria-label="Open details for {item.name}"
  >
    <p class="text-sm text-gray-200 truncate font-medium">{item.name}</p>
    {#if item.type === "Episode" && item.series_name}
      <p class="text-xs text-gray-400 truncate">{item.series_name}</p>
    {:else}
      <div class="flex items-center gap-1.5 text-xs text-gray-500">
        {#if item.production_year}
          <span>{item.production_year}</span>
        {/if}
        {#if item.community_rating}
          <span class="flex items-center gap-0.5">
            <svg class="w-3 h-3 text-yellow-500" viewBox="0 0 20 20" fill="currentColor">
              <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"/>
            </svg>
            {item.community_rating.toFixed(1)}
          </span>
        {/if}
        {#if item.run_time_ticks}
          <span>{formatRuntime(item.run_time_ticks)}</span>
        {/if}
      </div>
    {/if}
  </button>
</div>
