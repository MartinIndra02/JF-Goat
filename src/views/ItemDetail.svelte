<script lang="ts">
  import { onMount } from "svelte";
  import { push, querystring } from "svelte-spa-router";
  import {
    getItemById,
    getSeriesSeasons,
    getSeasonEpisodes,
  } from "../lib/api";
  import type { MediaItem } from "../lib/types";
  import PosterCard from "../components/media/PosterCard.svelte";

  // The item ID is passed as a query param: /item?id=xxx
  const params = new URLSearchParams($querystring);
  const itemId = params.get("id") ?? "";

  let item = $state<MediaItem | null>(null);
  let seasons = $state<MediaItem[]>([]);
  let episodes = $state<MediaItem[]>([]);
  let selectedSeasonId = $state<string | null>(null);
  let loading = $state(true);
  let episodesLoading = $state(false);

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

  function handleImageLoad(event: Event) {
    const img = event.target as HTMLImageElement;
    if (img.naturalWidth <= 1 && img.naturalHeight <= 1) {
      const src = img.src;
      const retryCount = parseInt(img.dataset.retry ?? "0");
      if (retryCount < 3) {
        setTimeout(() => {
          img.dataset.retry = String(retryCount + 1);
          img.src = "";
          img.src = src;
        }, 1500 * (retryCount + 1));
      }
    } else {
      img.classList.add("opacity-100");
    }
  }

  function navigateToItem(id: string) {
    push(`/item?id=${id}`);
  }

  function goBack() {
    push("/home");
  }

  async function loadSeasonEpisodes(seasonId: string) {
    selectedSeasonId = seasonId;
    episodesLoading = true;
    try {
      episodes = await getSeasonEpisodes(seasonId);
    } catch (e) {
      console.error("Failed to load episodes:", e);
      episodes = [];
    } finally {
      episodesLoading = false;
    }
  }

  onMount(async () => {
    if (!itemId) {
      push("/home");
      return;
    }

    try {
      const result = await getItemById(itemId);
      if (!result) {
        push("/home");
        return;
      }
      item = result;

      // If it's a series, load seasons
      if (item.type === "Series") {
        seasons = await getSeriesSeasons(item.id);
        // Auto-select first season
        if (seasons.length > 0) {
          await loadSeasonEpisodes(seasons[0].id);
        }
      }

      // If it's an episode, load sibling episodes from the same season
      if (item.type === "Episode" && item.season_id) {
        episodes = await getSeasonEpisodes(item.season_id);
      }
    } catch (e) {
      console.error("Failed to load item details:", e);
    } finally {
      loading = false;
    }
  });
</script>

{#if loading}
  <main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
    <div class="text-center">
      <svg class="w-8 h-8 text-blue-400 animate-spin mx-auto mb-3" viewBox="0 0 24 24" fill="none">
        <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25"/>
        <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
      </svg>
      <p class="text-gray-400 text-sm">Loading...</p>
    </div>
  </main>
{:else if item}
  <main class="min-h-screen bg-gray-900 text-white">
    <!-- Backdrop Hero Section -->
    <div class="relative w-full overflow-hidden" style="height: clamp(300px, 50vh, 500px);">
      {#if item.backdrop_tag}
        <img
          src={`http://jfimage.localhost/backdrop/${item.id}?tag=${item.backdrop_tag}`}
          alt={item.name}
          onload={handleImageLoad}
          class="absolute inset-0 w-full h-full object-cover transition-opacity duration-500 opacity-0"
        />
      {:else if item.type === "Episode" && item.series_id}
        <img
          src={`http://jfimage.localhost/backdrop/${item.series_id}?tag=${item.series_id}`}
          alt={item.name}
          onload={handleImageLoad}
          class="absolute inset-0 w-full h-full object-cover transition-opacity duration-500 opacity-0"
        />
      {:else}
        <div class="absolute inset-0 bg-gray-800"></div>
      {/if}

      <!-- Gradient overlays -->
      <div class="absolute inset-0 bg-gradient-to-t from-gray-900 via-gray-900/60 to-transparent"></div>
      <div class="absolute inset-0 bg-gradient-to-r from-gray-900/90 via-gray-900/30 to-transparent"></div>

      <!-- Back button -->
      <button
        onclick={goBack}
        class="absolute top-4 left-4 z-10 flex items-center gap-2 px-3 py-2 bg-black/50 hover:bg-black/70 rounded-lg backdrop-blur-sm transition-colors"
      >
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
        </svg>
        <span class="text-sm">Back</span>
      </button>
    </div>

    <!-- Content Section -->
    <div class="relative -mt-32 z-10 px-6 pb-16 max-w-6xl mx-auto">
      <div class="flex flex-col sm:flex-row gap-6">
        <!-- Poster -->
        <div class="flex-shrink-0 w-40 sm:w-48">
          <div class="relative overflow-hidden rounded-lg shadow-2xl">
            {#if item.image_tag}
              <img
                src={`http://jfimage.localhost/poster/${item.id}?tag=${item.image_tag}`}
                alt={item.name}
                onload={handleImageLoad}
                class="w-full aspect-[2/3] object-cover transition-opacity duration-300 opacity-0"
              />
            {:else if item.series_id}
              <img
                src={`http://jfimage.localhost/poster/${item.series_id}?tag=${item.series_id}`}
                alt={item.name}
                onload={handleImageLoad}
                class="w-full aspect-[2/3] object-cover transition-opacity duration-300 opacity-0"
              />
            {:else}
              <div class="w-full aspect-[2/3] bg-gray-800 flex items-center justify-center">
                <span class="text-gray-400 text-sm text-center px-4">{item.name}</span>
              </div>
            {/if}
            <div class="absolute inset-0 bg-gray-800 -z-10"></div>

            <!-- Progress bar -->
            {#if progressPercent(item) > 0}
              <div class="absolute bottom-0 left-0 right-0 h-1.5 bg-black/50">
                <div
                  class="h-full bg-blue-500 rounded-r-full"
                  style="width: {progressPercent(item)}%"
                ></div>
              </div>
            {/if}
          </div>
        </div>

        <!-- Details -->
        <div class="flex-1 min-w-0">
          <!-- Episode context -->
          {#if item.type === "Episode" && item.series_name}
            <button
              onclick={() => item?.series_id && navigateToItem(item.series_id)}
              class="text-blue-400 hover:text-blue-300 text-sm font-medium mb-1 transition-colors cursor-pointer"
            >
              {item.series_name}
            </button>
            {#if item.season_name}
              <p class="text-gray-400 text-sm mb-1">
                {item.season_name} · Episode {item.index_number ?? "?"}
              </p>
            {/if}
          {/if}

          <!-- Title -->
          <h1 class="text-3xl sm:text-4xl font-bold text-white mb-3 leading-tight">
            {item.name}
          </h1>

          <!-- Metadata row -->
          <div class="flex flex-wrap items-center gap-3 mb-4">
            {#if item.type !== "Episode"}
              <span class="text-xs font-medium text-blue-400 bg-blue-400/10 px-2.5 py-1 rounded">
                {item.type}
              </span>
            {/if}

            {#if item.official_rating}
              <span class="text-xs font-medium text-gray-300 bg-white/10 px-2.5 py-1 rounded border border-white/20">
                {item.official_rating}
              </span>
            {/if}

            {#if item.production_year}
              <span class="flex items-center gap-1 text-sm text-gray-400">
                <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clip-rule="evenodd"/>
                </svg>
                {item.production_year}
              </span>
            {/if}

            {#if item.run_time_ticks}
              <span class="flex items-center gap-1 text-sm text-gray-400">
                <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd"/>
                </svg>
                {formatRuntime(item.run_time_ticks)}
              </span>
            {/if}

            {#if item.community_rating}
              <span class="flex items-center gap-1 text-sm text-yellow-400 bg-yellow-400/10 px-2.5 py-1 rounded">
                <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
                  <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"/>
                </svg>
                {item.community_rating.toFixed(1)}
              </span>
            {/if}

            {#if item.played}
              <span class="flex items-center gap-1 text-sm text-green-400">
                <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/>
                </svg>
                Watched
              </span>
            {/if}

            {#if item.is_favorite}
              <span class="flex items-center gap-1 text-sm text-red-400">
                <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/>
                </svg>
                Favorite
              </span>
            {/if}
          </div>

          <!-- Genres -->
          {#if item.genres}
            <div class="flex flex-wrap gap-2 mb-5">
              {#each item.genres.split(",").slice(0, 6) as genre}
                <span class="text-xs text-gray-300 bg-white/10 px-3 py-1 rounded-full">
                  {genre.trim()}
                </span>
              {/each}
            </div>
          {/if}

          <!-- Overview -->
          {#if item.overview}
            <div class="mb-6">
              <p class="text-gray-300 text-sm leading-relaxed">{item.overview}</p>
            </div>
          {/if}
        </div>
      </div>

      <!-- Series: Seasons and Episodes -->
      {#if item.type === "Series" && seasons.length > 0}
        <div class="mt-8">
          <!-- Season Tabs -->
          <div class="flex gap-2 overflow-x-auto pb-3 scrollbar-hide mb-4">
            {#each seasons as season (season.id)}
              <button
                onclick={() => loadSeasonEpisodes(season.id)}
                class="flex-shrink-0 px-4 py-2 rounded-lg text-sm font-medium transition-colors {
                  selectedSeasonId === season.id
                    ? 'bg-blue-600 text-white'
                    : 'bg-white/10 text-gray-300 hover:bg-white/20'
                }"
              >
                {season.name}
              </button>
            {/each}
          </div>

          <!-- Episodes Grid -->
          {#if episodesLoading}
            <div class="flex items-center justify-center h-32">
              <svg class="w-6 h-6 text-blue-400 animate-spin" viewBox="0 0 24 24" fill="none">
                <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25"/>
                <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
              </svg>
            </div>
          {:else if episodes.length > 0}
            <div class="space-y-2">
              {#each episodes as episode (episode.id)}
                <button
                  onclick={() => navigateToItem(episode.id)}
                  class="w-full flex gap-4 p-3 rounded-lg bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer"
                >
                  <!-- Episode Thumbnail -->
                  <div class="flex-shrink-0 w-36 sm:w-44 relative overflow-hidden rounded-md">
                    {#if episode.backdrop_tag}
                      <img
                        src={`http://jfimage.localhost/backdrop/${episode.id}?tag=${episode.backdrop_tag}`}
                        alt={episode.name}
                        onload={handleImageLoad}
                        class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0"
                      />
                    {:else if episode.image_tag}
                      <img
                        src={`http://jfimage.localhost/poster/${episode.id}?tag=${episode.image_tag}`}
                        alt={episode.name}
                        onload={handleImageLoad}
                        class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0"
                      />
                    {:else}
                      <div class="w-full aspect-video bg-gray-800 flex items-center justify-center">
                        <span class="text-gray-500 text-xs">Ep {episode.index_number ?? "?"}</span>
                      </div>
                    {/if}
                    <div class="absolute inset-0 bg-gray-800 -z-10"></div>

                    {#if progressPercent(episode) > 0}
                      <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
                        <div
                          class="h-full bg-blue-500 rounded-r-full"
                          style="width: {progressPercent(episode)}%"
                        ></div>
                      </div>
                    {/if}
                  </div>

                  <!-- Episode Info -->
                  <div class="flex-1 min-w-0 py-1">
                    <div class="flex items-center gap-2 mb-1">
                      <span class="text-xs text-gray-500 font-medium">
                        E{episode.index_number ?? "?"}
                      </span>
                      {#if episode.run_time_ticks}
                        <span class="text-xs text-gray-500">
                          {formatRuntime(episode.run_time_ticks)}
                        </span>
                      {/if}
                      {#if episode.played}
                        <svg class="w-3.5 h-3.5 text-green-500" viewBox="0 0 20 20" fill="currentColor">
                          <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/>
                        </svg>
                      {/if}
                    </div>
                    <p class="text-sm text-white font-medium truncate">{episode.name}</p>
                    {#if episode.overview}
                      <p class="text-xs text-gray-400 line-clamp-2 mt-1">{episode.overview}</p>
                    {/if}
                  </div>
                </button>
              {/each}
            </div>
          {:else}
            <p class="text-gray-500 text-sm text-center py-8">No episodes found for this season.</p>
          {/if}
        </div>
      {/if}

      <!-- Episode: Other episodes in the same season -->
      {#if item.type === "Episode" && episodes.length > 1}
        <div class="mt-8">
          <h2 class="text-lg font-semibold text-white mb-3">
            {item.season_name ?? "Other Episodes"}
          </h2>
          <div class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide">
            {#each episodes as episode (episode.id)}
              <button
                onclick={() => navigateToItem(episode.id)}
                class="flex-shrink-0 w-48 rounded-lg overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer {
                  episode.id === item.id ? 'ring-2 ring-blue-500' : ''
                }"
              >
                <div class="relative">
                  {#if episode.backdrop_tag}
                    <img
                      src={`http://jfimage.localhost/backdrop/${episode.id}?tag=${episode.backdrop_tag}`}
                      alt={episode.name}
                      onload={handleImageLoad}
                      class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0"
                    />
                  {:else}
                    <div class="w-full aspect-video bg-gray-800 flex items-center justify-center">
                      <span class="text-gray-500 text-xs">Ep {episode.index_number ?? "?"}</span>
                    </div>
                  {/if}
                  <div class="absolute inset-0 bg-gray-800 -z-10"></div>
                  {#if progressPercent(episode) > 0}
                    <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
                      <div
                        class="h-full bg-blue-500 rounded-r-full"
                        style="width: {progressPercent(episode)}%"
                      ></div>
                    </div>
                  {/if}
                </div>
                <div class="p-2">
                  <p class="text-xs text-gray-500">E{episode.index_number ?? "?"}</p>
                  <p class="text-sm text-white truncate">{episode.name}</p>
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  </main>
{/if}
