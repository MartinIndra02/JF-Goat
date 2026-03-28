<script lang="ts">
  import {
    getSeasonEpisodes,
    getSeriesSeasons,
    mpvPlay,
  } from "../../lib/api";
  import {
    getPreferredAudioStreamIndex,
    getPreferredSubtitleStreamIndex,
    showPlayer,
  } from "../../lib/stores/player.svelte";
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

  const progress = $derived(progressPercent(item));
  const aspectClass = $derived(landscape ? "aspect-video" : "aspect-[2/3]");
  let launchingPlayback = $state(false);

  function openDetail() {
    push(`/item?id=${item.id}`);
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

  // Retry loading images that were returned as transparent placeholders (cache miss).
  // The background fetch will populate the cache, so the retry will succeed.
  function handleImageLoad(event: Event) {
    const img = event.target as HTMLImageElement;
    if (img.naturalWidth <= 1 && img.naturalHeight <= 1) {
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
      // Real image loaded — fade it in
      img.classList.add("opacity-100");
    }
  }
</script>

<div
  class="group flex-shrink-0 {landscape ? 'w-56 sm:w-64' : 'w-32 sm:w-36'}"
>
  <button
    type="button"
    onclick={handlePosterClick}
    class="block w-full text-left relative overflow-hidden rounded-lg shadow-md transition-transform duration-200 group-hover:scale-105 group-hover:shadow-xl cursor-pointer"
    aria-label="Play {item.name}"
  >
    {#if landscape && item.backdrop_tag}
      <img
        src={`http://jfimage.localhost/backdrop/${item.id}?tag=${item.backdrop_tag}`}
        alt={item.name}
        loading="lazy"
        onload={handleImageLoad}
        class="w-full {aspectClass} object-cover transition-opacity duration-300 opacity-0"
      />
    {:else if item.image_tag}
      <img
        src={`http://jfimage.localhost/poster/${item.id}?tag=${item.image_tag}`}
        alt={item.name}
        loading="lazy"
        onload={handleImageLoad}
        class="w-full {aspectClass} object-cover transition-opacity duration-300 opacity-0"
      />
    {:else if item.series_id}
      <img
        src={`http://jfimage.localhost/poster/${item.series_id}?tag=${item.series_id}`}
        alt={item.name}
        loading="lazy"
        onload={handleImageLoad}
        class="w-full {aspectClass} object-cover transition-opacity duration-300 opacity-0"
      />
    {:else}
      <div class="w-full {aspectClass} bg-gray-800 flex items-center justify-center">
        <span class="text-gray-400 text-xs text-center px-2 line-clamp-3">{item.name}</span>
      </div>
    {/if}

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

  <button
    type="button"
    onclick={openDetail}
    class="mt-1.5 px-0.5 block w-full text-left cursor-pointer"
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
