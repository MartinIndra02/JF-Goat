<script lang="ts">
  import { onMount } from "svelte";
  import { push, querystring } from "svelte-spa-router";
  import {
    getItemById,
    getSeriesSeasons,
    getSeasonEpisodes,
  } from "../lib/api";
  import type { MediaItem } from "../lib/types";

  // The item ID is passed as a query param: /item?id=xxx
  const params = new URLSearchParams($querystring);
  const itemId = params.get("id") ?? "";

  let item = $state<MediaItem | null>(null);
  let seasons = $state<MediaItem[]>([]);
  let episodes = $state<MediaItem[]>([]);
  let selectedSeasonId = $state<string | null>(null);
  let allSeasonEpisodes = $state<Record<string, MediaItem[]>>({});
  let loading = $state(true);
  let episodesLoading = $state(false);
  let overviewExpanded = $state(false);
  let seasonEpisodesViewMode = $state<"list" | "grid">("list");

  // For episode detail: sibling episodes from same season
  let siblingEpisodes = $state<MediaItem[]>([]);

  const IMAGE_BASE = "http://jfimage.localhost";

  // Derived: resume episode for series detail
  const resumeEpisode = $derived.by(() => {
    // Look through all cached episode lists for an in-progress episode
    for (const eps of Object.values(allSeasonEpisodes)) {
      for (const ep of eps) {
        if (ep.playback_ticks > 0 && !ep.played) {
          return ep;
        }
      }
    }
    // Find first unwatched episode
    for (const eps of Object.values(allSeasonEpisodes)) {
      for (const ep of eps) {
        if (!ep.played) {
          return ep;
        }
      }
    }
    return null;
  });

  /** Extract the season number from a season name (e.g. "Season 6" → "6") */
  function seasonNumber(seasonName: string | null | undefined): string {
    return seasonName?.replace("Season ", "") ?? "?";
  }

  function formatRuntime(ticks: number | null): string {
    if (!ticks) return "";
    const minutes = Math.round(ticks / 600_000_000);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  }

  function formatDate(dateStr: string | null): string {
    if (!dateStr) return "";
    try {
      const d = new Date(dateStr);
      return d.toLocaleDateString("en-US", { year: "numeric", month: "short", day: "numeric" });
    } catch {
      return "";
    }
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
    window.history.length > 1 ? window.history.back() : push("/home");
  }

  async function loadSeasonEpisodes(seasonId: string) {
    selectedSeasonId = seasonId;
    // Check cache first
    if (allSeasonEpisodes[seasonId]) {
      episodes = allSeasonEpisodes[seasonId];
      return;
    }
    episodesLoading = true;
    try {
      const eps = await getSeasonEpisodes(seasonId);
      allSeasonEpisodes[seasonId] = eps;
      episodes = eps;
    } catch (e) {
      console.error("Failed to load episodes:", e);
      episodes = [];
    } finally {
      episodesLoading = false;
    }
  }

  const selectedSeason = $derived(seasons.find(s => s.id === selectedSeasonId) ?? null);

  function backdropUrl(itm: MediaItem): string {
    if (itm.backdrop_tag) {
      return `${IMAGE_BASE}/backdrop/${itm.id}?tag=${itm.backdrop_tag}`;
    }
    if (itm.series_id) {
      return `${IMAGE_BASE}/backdrop/${itm.series_id}?tag=${itm.series_id}`;
    }
    return "";
  }

  function posterUrl(itm: MediaItem): string {
    if (itm.image_tag) {
      return `${IMAGE_BASE}/poster/${itm.id}?tag=${itm.image_tag}`;
    }
    if (itm.series_id) {
      return `${IMAGE_BASE}/poster/${itm.series_id}?tag=${itm.series_id}`;
    }
    return "";
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

      // Series: load seasons and first season episodes
      if (item.type === "Series") {
        seasons = await getSeriesSeasons(item.id);
        if (seasons.length > 0) {
          await loadSeasonEpisodes(seasons[0].id);
        }
      }

      // Season: load episodes for this season and parent series info
      if (item.type === "Season") {
        const eps = await getSeasonEpisodes(item.id);
        episodes = eps;
        // Also load sibling seasons
        if (item.series_id) {
          seasons = await getSeriesSeasons(item.series_id);
        }
      }

      // Episode/Movie: load sibling episodes from same season
      if (item.type === "Episode" && item.season_id) {
        siblingEpisodes = await getSeasonEpisodes(item.season_id);
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

<!-- ════════════════════════════════════════════════════════════════════
     EPISODE / MOVIE DETAIL
     ════════════════════════════════════════════════════════════════════ -->
{:else if item && (item.type === "Episode" || item.type === "Movie")}
  <main class="min-h-screen bg-gray-900 text-white">
    <!-- Hero Backdrop -->
    <div class="relative w-full overflow-hidden" style="height: clamp(280px, 45vh, 480px);">
      {#if backdropUrl(item)}
        <img
          src={backdropUrl(item)}
          alt=""
          onload={handleImageLoad}
          class="absolute inset-0 w-full h-full object-cover transition-opacity duration-500 opacity-0"
        />
      {/if}
      <div class="absolute inset-0 bg-gray-800 -z-10"></div>
      <div class="absolute inset-0 bg-gradient-to-t from-gray-900 via-gray-900/50 to-transparent"></div>
      <div class="absolute inset-0 bg-gradient-to-r from-gray-900/80 via-transparent to-transparent"></div>

      <!-- Back button -->
      <button
        onclick={goBack}
        class="absolute top-4 left-4 z-10 flex items-center gap-2 px-3 py-2 bg-black/50 hover:bg-black/70 rounded-lg backdrop-blur-sm transition-colors"
      >
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
        </svg>
        <span class="text-sm font-medium">Back</span>
      </button>
    </div>

    <div class="relative -mt-28 z-10 px-6 pb-16 max-w-5xl mx-auto">
      <!-- Episode/Movie Header Info -->
      <div class="mb-4">
        {#if item.type === "Episode" && item.series_name}
          <button
            onclick={() => item?.series_id && navigateToItem(item.series_id)}
            class="text-blue-400 hover:text-blue-300 text-sm font-semibold mb-1 transition-colors cursor-pointer inline-flex items-center gap-1"
          >
            <span>{item.series_name}</span>
            <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M7.21 14.77a.75.75 0 01.02-1.06L11.168 10 7.23 6.29a.75.75 0 111.04-1.08l4.5 4.25a.75.75 0 010 1.08l-4.5 4.25a.75.75 0 01-1.06-.02z" clip-rule="evenodd"/></svg>
          </button>
        {/if}

        <!-- Title: S6 - E2 - Home or Movie title -->
        <h1 class="text-2xl sm:text-3xl font-bold text-white leading-tight mb-2">
          {#if item.type === "Episode"}
            {#if item.season_name && item.index_number}
              <span class="text-gray-400 font-medium">S{seasonNumber(item.season_name)} · E{item.index_number}</span>
              <span class="text-gray-600 mx-1">—</span>
            {/if}
          {/if}
          {item.name}
        </h1>

        <!-- Metadata row -->
        <div class="flex flex-wrap items-center gap-2.5 mb-4 text-sm">
          {#if item.official_rating}
            <span class="text-xs font-semibold text-gray-200 bg-white/10 px-2 py-0.5 rounded border border-white/20">
              {item.official_rating}
            </span>
          {/if}

          {#if item.date_created && item.type === "Episode"}
            <!-- date_created maps to the episode's premiere/air date from Jellyfin -->
            <span class="text-gray-400 flex items-center gap-1">
              <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
                <path fill-rule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clip-rule="evenodd"/>
              </svg>
              {formatDate(item.date_created)}
            </span>
          {:else if item.production_year}
            <span class="text-gray-400">{item.production_year}</span>
          {/if}

          {#if item.run_time_ticks}
            <span class="text-gray-400 flex items-center gap-1">
              <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
                <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd"/>
              </svg>
              {formatRuntime(item.run_time_ticks)}
            </span>
          {/if}

          {#if item.community_rating}
            <span class="flex items-center gap-1 text-yellow-400">
              <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
                <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"/>
              </svg>
              {item.community_rating.toFixed(1)}
            </span>
          {/if}
        </div>

        <!-- Genre badges -->
        {#if item.genres}
          <div class="flex flex-wrap gap-1.5 mb-5">
            {#each item.genres.split(",").slice(0, 6) as genre}
              <span class="text-xs text-gray-300 bg-white/10 px-2.5 py-1 rounded-full">
                {genre.trim()}
              </span>
            {/each}
          </div>
        {/if}

        <!-- Overview -->
        {#if item.overview}
          <p class="text-gray-300 text-sm leading-relaxed mb-5">{item.overview}</p>
        {/if}
      </div>

      <!-- Primary Actions -->
      <div class="flex flex-wrap items-center gap-3 mb-6">
        <!-- Play / Resume button -->
        <button class="relative flex items-center gap-2 px-5 py-2.5 bg-blue-600 hover:bg-blue-700 rounded-lg font-semibold text-sm transition-colors overflow-hidden">
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
            <path d="M8 5v14l11-7z"/>
          </svg>
          {#if progressPercent(item) > 0}
            <span>Resume</span>
            <!-- Progress bar at bottom of button -->
            <div class="absolute bottom-0 left-0 right-0 h-1 bg-blue-900">
              <div class="h-full bg-blue-300" style="width: {progressPercent(item)}%"></div>
            </div>
          {:else}
            <span>Play</span>
          {/if}
        </button>

        {#if progressPercent(item) > 0}
          <button class="flex items-center gap-2 px-4 py-2.5 bg-white/10 hover:bg-white/20 rounded-lg text-sm transition-colors">
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
            </svg>
            <span>Play from start</span>
          </button>
        {/if}

        <div class="flex items-center gap-1 ml-auto">
          <!-- Favorite -->
          <button aria-label="Toggle favorite" class="p-2.5 rounded-lg hover:bg-white/10 transition-colors {item.is_favorite ? 'text-red-400' : 'text-gray-400'}">
            <svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor">
              <path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/>
            </svg>
          </button>

          <!-- Watched -->
          <button aria-label="Toggle watched" class="p-2.5 rounded-lg hover:bg-white/10 transition-colors {item.played ? 'text-green-400' : 'text-gray-400'}">
            <svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor">
              <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
            </svg>
          </button>
        </div>
      </div>

      <!-- Divider -->
      <div class="border-t border-white/10 mb-6"></div>

      <!-- More from season X (for episodes) -->
      {#if item.type === "Episode" && siblingEpisodes.length > 1}
        <div class="mb-8">
          <h2 class="text-lg font-semibold text-white mb-3">
            More from {item.season_name ?? "this season"}
          </h2>
          <div class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-6 px-6">
            {#each siblingEpisodes as episode (episode.id)}
              <button
                onclick={() => navigateToItem(episode.id)}
                class="flex-shrink-0 w-52 rounded-lg overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer {
                  episode.id === item?.id ? 'ring-2 ring-blue-500' : ''
                }"
              >
                <div class="relative">
                  {#if episode.backdrop_tag}
                    <img
                      src={`${IMAGE_BASE}/backdrop/${episode.id}?tag=${episode.backdrop_tag}`}
                      alt={episode.name}
                      onload={handleImageLoad}
                      class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0"
                    />
                  {:else}
                    <div class="w-full aspect-video bg-gray-800 flex items-center justify-center">
                      <span class="text-gray-500 text-xs">E{episode.index_number ?? "?"}</span>
                    </div>
                  {/if}
                  <div class="absolute inset-0 bg-gray-800 -z-10"></div>

                  {#if episode.played}
                    <div class="absolute top-1.5 right-1.5 bg-green-500/90 rounded-full p-0.5">
                      <svg class="w-3 h-3 text-white" viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
                      </svg>
                    </div>
                  {/if}

                  {#if progressPercent(episode) > 0}
                    <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
                      <div class="h-full bg-blue-500 rounded-r-full" style="width: {progressPercent(episode)}%"></div>
                    </div>
                  {/if}
                </div>
                <div class="p-2.5">
                  <p class="text-xs text-gray-500 mb-0.5">
                    S{seasonNumber(item?.season_name)} · E{episode.index_number ?? "?"}
                    {#if episode.run_time_ticks}
                      <span class="ml-1">{formatRuntime(episode.run_time_ticks)}</span>
                    {/if}
                  </p>
                  <p class="text-sm text-white truncate font-medium">{episode.name}</p>
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  </main>

<!-- ════════════════════════════════════════════════════════════════════
     SERIES DETAIL
     ════════════════════════════════════════════════════════════════════ -->
{:else if item && item.type === "Series"}
  <main class="min-h-screen bg-gray-900 text-white">
    <!-- Hero Backdrop -->
    <div class="relative w-full overflow-hidden" style="height: clamp(280px, 45vh, 480px);">
      {#if backdropUrl(item)}
        <img
          src={backdropUrl(item)}
          alt=""
          onload={handleImageLoad}
          class="absolute inset-0 w-full h-full object-cover transition-opacity duration-500 opacity-0"
        />
      {/if}
      <div class="absolute inset-0 bg-gray-800 -z-10"></div>
      <div class="absolute inset-0 bg-gradient-to-t from-gray-900 via-gray-900/50 to-transparent"></div>
      <div class="absolute inset-0 bg-gradient-to-r from-gray-900/80 via-transparent to-transparent"></div>

      <!-- Back button -->
      <button
        onclick={goBack}
        class="absolute top-4 left-4 z-10 flex items-center gap-2 px-3 py-2 bg-black/50 hover:bg-black/70 rounded-lg backdrop-blur-sm transition-colors"
      >
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
        </svg>
        <span class="text-sm font-medium">Back</span>
      </button>
    </div>

    <div class="relative -mt-28 z-10 px-6 pb-16 max-w-5xl mx-auto">
      <!-- Series Title -->
      <h1 class="text-3xl sm:text-4xl font-bold text-white leading-tight mb-3">
        {item.name}
      </h1>

      <!-- Metadata row -->
      <div class="flex flex-wrap items-center gap-2.5 mb-4 text-sm">
        {#if item.official_rating}
          <span class="text-xs font-semibold text-gray-200 bg-white/10 px-2 py-0.5 rounded border border-white/20">
            {item.official_rating}
          </span>
        {/if}

        {#if item.production_year}
          <span class="text-gray-400">{item.production_year}</span>
        {/if}

        {#if item.run_time_ticks}
          <span class="text-gray-400 flex items-center gap-1">
            <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
              <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd"/>
            </svg>
            {formatRuntime(item.run_time_ticks)}
          </span>
        {/if}

        {#if item.community_rating}
          <span class="flex items-center gap-1 text-yellow-400">
            <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
              <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"/>
            </svg>
            {item.community_rating.toFixed(1)}
          </span>
        {/if}
      </div>

      <!-- Genre badges -->
      {#if item.genres}
        <div class="flex flex-wrap gap-1.5 mb-5">
          {#each item.genres.split(",").slice(0, 6) as genre}
            <span class="text-xs text-gray-300 bg-white/10 px-2.5 py-1 rounded-full">
              {genre.trim()}
            </span>
          {/each}
        </div>
      {/if}

      <!-- Overview with expand/collapse -->
      {#if item.overview}
        <div class="mb-5">
          <p class="text-gray-300 text-sm leading-relaxed {overviewExpanded ? '' : 'line-clamp-3'}">
            {item.overview}
          </p>
          {#if item.overview.length > 200}
            <button
              onclick={() => overviewExpanded = !overviewExpanded}
              class="text-blue-400 hover:text-blue-300 text-xs font-medium mt-1 transition-colors"
            >
              {overviewExpanded ? "Show less" : "Read more"}
            </button>
          {/if}
        </div>
      {/if}

      <!-- Dynamic Action Panel -->
      <div class="flex flex-wrap items-center gap-3 mb-6">
        {#if resumeEpisode}
          <button
            onclick={() => resumeEpisode && navigateToItem(resumeEpisode.id)}
            class="relative flex items-center gap-2 px-5 py-2.5 bg-blue-600 hover:bg-blue-700 rounded-lg font-semibold text-sm transition-colors overflow-hidden"
          >
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z"/>
            </svg>
            <span>
              {progressPercent(resumeEpisode) > 0 ? "Resume" : "Play"}
              S{seasonNumber(resumeEpisode.season_name)} · E{resumeEpisode.index_number ?? "?"}
            </span>
            {#if progressPercent(resumeEpisode) > 0}
              <div class="absolute bottom-0 left-0 right-0 h-1 bg-blue-900">
                <div class="h-full bg-blue-300" style="width: {progressPercent(resumeEpisode)}%"></div>
              </div>
            {/if}
          </button>
        {:else}
          <button class="flex items-center gap-2 px-5 py-2.5 bg-blue-600 hover:bg-blue-700 rounded-lg font-semibold text-sm transition-colors">
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z"/>
            </svg>
            <span>Play</span>
          </button>
        {/if}

        <div class="flex items-center gap-1 ml-auto">
          <!-- Favorite -->
          <button aria-label="Toggle favorite" class="p-2.5 rounded-lg hover:bg-white/10 transition-colors {item.is_favorite ? 'text-red-400' : 'text-gray-400'}">
            <svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor">
              <path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/>
            </svg>
          </button>

          <!-- Watched -->
          <button aria-label="Toggle watched" class="p-2.5 rounded-lg hover:bg-white/10 transition-colors {item.played ? 'text-green-400' : 'text-gray-400'}">
            <svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor">
              <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
            </svg>
          </button>
        </div>
      </div>

      <div class="border-t border-white/10 mb-6"></div>

      <!-- Episodes section with season dropdown -->
      {#if seasons.length > 0}
        <div class="mb-8">
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-lg font-semibold text-white">Episodes</h2>

            <!-- Season dropdown -->
            <div class="relative">
              <select
                onchange={(e) => {
                  const target = e.target as HTMLSelectElement;
                  loadSeasonEpisodes(target.value);
                }}
                value={selectedSeasonId ?? ""}
                class="appearance-none bg-white/10 text-white text-sm px-3 py-1.5 pr-8 rounded-lg border border-white/10 focus:outline-none focus:ring-2 focus:ring-blue-500 cursor-pointer"
              >
                {#each seasons as season (season.id)}
                  <option value={season.id} class="bg-gray-800">{season.name}</option>
                {/each}
              </select>
              <svg class="absolute right-2 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 pointer-events-none" viewBox="0 0 20 20" fill="currentColor">
                <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/>
              </svg>
            </div>
          </div>

          {#if episodesLoading}
            <div class="flex items-center justify-center h-24">
              <svg class="w-6 h-6 text-blue-400 animate-spin" viewBox="0 0 24 24" fill="none">
                <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25"/>
                <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
              </svg>
            </div>
          {:else if episodes.length > 0}
            <!-- Episode carousel -->
            <div class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-6 px-6">
              {#each episodes as episode (episode.id)}
                <button
                  onclick={() => navigateToItem(episode.id)}
                  class="flex-shrink-0 w-52 rounded-lg overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer"
                >
                  <div class="relative">
                    {#if episode.backdrop_tag}
                      <img
                        src={`${IMAGE_BASE}/backdrop/${episode.id}?tag=${episode.backdrop_tag}`}
                        alt={episode.name}
                        onload={handleImageLoad}
                        class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0"
                      />
                    {:else}
                      <div class="w-full aspect-video bg-gray-800 flex items-center justify-center">
                        <span class="text-gray-500 text-xs">E{episode.index_number ?? "?"}</span>
                      </div>
                    {/if}
                    <div class="absolute inset-0 bg-gray-800 -z-10"></div>

                    {#if episode.played}
                      <div class="absolute top-1.5 right-1.5 bg-green-500/90 rounded-full p-0.5">
                        <svg class="w-3 h-3 text-white" viewBox="0 0 20 20" fill="currentColor">
                          <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
                        </svg>
                      </div>
                    {/if}

                    {#if progressPercent(episode) > 0}
                      <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
                        <div class="h-full bg-blue-500 rounded-r-full" style="width: {progressPercent(episode)}%"></div>
                      </div>
                    {/if}
                  </div>
                  <div class="p-2.5">
                    <p class="text-xs text-gray-500 mb-0.5">
                      S{seasonNumber(selectedSeason?.name)} · E{episode.index_number ?? "?"}
                      {#if episode.run_time_ticks}
                        <span class="ml-1">{formatRuntime(episode.run_time_ticks)}</span>
                      {/if}
                    </p>
                    <p class="text-sm text-white truncate font-medium">{episode.name}</p>
                  </div>
                </button>
              {/each}
            </div>
          {:else}
            <p class="text-gray-500 text-sm text-center py-6">No episodes found for this season.</p>
          {/if}
        </div>
      {/if}

      <!-- Seasons carousel -->
      {#if seasons.length > 1}
        <div class="mb-8">
          <h2 class="text-lg font-semibold text-white mb-3">Seasons</h2>
          <div class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-6 px-6">
            {#each seasons as season (season.id)}
              <button
                onclick={() => navigateToItem(season.id)}
                class="flex-shrink-0 w-28 rounded-lg overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer group"
              >
                <div class="relative">
                  {#if season.image_tag}
                    <img
                      src={`${IMAGE_BASE}/poster/${season.id}?tag=${season.image_tag}`}
                      alt={season.name}
                      onload={handleImageLoad}
                      class="w-full aspect-[2/3] object-cover transition-opacity duration-300 opacity-0"
                    />
                  {:else}
                    <div class="w-full aspect-[2/3] bg-gray-800 flex items-center justify-center">
                      <span class="text-gray-500 text-xs text-center px-2">{season.name}</span>
                    </div>
                  {/if}
                  <div class="absolute inset-0 bg-gray-800 -z-10"></div>

                  {#if season.played}
                    <div class="absolute top-1.5 right-1.5 bg-green-500/90 rounded-full p-0.5">
                      <svg class="w-3 h-3 text-white" viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
                      </svg>
                    </div>
                  {/if}
                </div>
                <div class="p-2">
                  <p class="text-xs text-white truncate font-medium">{season.name}</p>
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  </main>

<!-- ════════════════════════════════════════════════════════════════════
     SEASON DETAIL
     ════════════════════════════════════════════════════════════════════ -->
{:else if item && item.type === "Season"}
  <main class="min-h-screen bg-gray-900 text-white">
    <!-- Hero Backdrop (from parent series) -->
    <div class="relative w-full overflow-hidden" style="height: clamp(240px, 40vh, 420px);">
      {#if backdropUrl(item)}
        <img
          src={backdropUrl(item)}
          alt=""
          onload={handleImageLoad}
          class="absolute inset-0 w-full h-full object-cover transition-opacity duration-500 opacity-0"
        />
      {/if}
      <div class="absolute inset-0 bg-gray-800 -z-10"></div>
      <div class="absolute inset-0 bg-gradient-to-t from-gray-900 via-gray-900/50 to-transparent"></div>

      <!-- Back button -->
      <button
        onclick={goBack}
        class="absolute top-4 left-4 z-10 flex items-center gap-2 px-3 py-2 bg-black/50 hover:bg-black/70 rounded-lg backdrop-blur-sm transition-colors"
      >
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
        </svg>
        <span class="text-sm font-medium">Back</span>
      </button>
    </div>

    <div class="relative -mt-24 z-10 px-6 pb-16 max-w-5xl mx-auto">
      <!-- Series name link -->
      {#if item.series_name && item.series_id}
        <button
          onclick={() => item?.series_id && navigateToItem(item.series_id)}
          class="text-blue-400 hover:text-blue-300 text-sm font-semibold mb-1 transition-colors cursor-pointer inline-flex items-center gap-1"
        >
          <span>{item.series_name}</span>
          <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M7.21 14.77a.75.75 0 01.02-1.06L11.168 10 7.23 6.29a.75.75 0 111.04-1.08l4.5 4.25a.75.75 0 010 1.08l-4.5 4.25a.75.75 0 01-1.06-.02z" clip-rule="evenodd"/></svg>
        </button>
      {/if}

      <!-- Season heading -->
      <h1 class="text-3xl font-bold text-white mb-3">{item.name}</h1>

      <!-- Actions row -->
      <div class="flex flex-wrap items-center gap-3 mb-5">
        <!-- Favorite -->
        <button class="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-white/10 hover:bg-white/15 transition-colors text-sm {item.is_favorite ? 'text-red-400' : 'text-gray-300'}">
          <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/>
          </svg>
          <span>Favorite</span>
        </button>

        <!-- Mark all as watched -->
        <button class="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-white/10 hover:bg-white/15 transition-colors text-sm {item.played ? 'text-green-400' : 'text-gray-300'}">
          <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
          </svg>
          <span>Watched</span>
        </button>

        <!-- View toggle -->
        <div class="flex items-center ml-auto bg-white/5 rounded-lg overflow-hidden border border-white/10">
          <button
            onclick={() => seasonEpisodesViewMode = "list"}
            class="px-3 py-1.5 text-xs font-medium transition-colors {seasonEpisodesViewMode === 'list' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-white'}"
          >
            List
          </button>
          <button
            onclick={() => seasonEpisodesViewMode = "grid"}
            class="px-3 py-1.5 text-xs font-medium transition-colors {seasonEpisodesViewMode === 'grid' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-white'}"
          >
            Grid
          </button>
        </div>
      </div>

      <!-- Season overview -->
      {#if item.overview}
        <p class="text-gray-300 text-sm leading-relaxed mb-5">{item.overview}</p>
      {/if}

      <div class="border-t border-white/10 mb-5"></div>

      <!-- Episodes list or grid -->
      {#if episodes.length > 0}
        {#if seasonEpisodesViewMode === "list"}
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
                      src={`${IMAGE_BASE}/backdrop/${episode.id}?tag=${episode.backdrop_tag}`}
                      alt={episode.name}
                      onload={handleImageLoad}
                      class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0"
                    />
                  {:else}
                    <div class="w-full aspect-video bg-gray-800 flex items-center justify-center">
                      <span class="text-gray-500 text-xs">E{episode.index_number ?? "?"}</span>
                    </div>
                  {/if}
                  <div class="absolute inset-0 bg-gray-800 -z-10"></div>

                  {#if episode.played}
                    <div class="absolute top-1 right-1 bg-green-500/90 rounded-full p-0.5">
                      <svg class="w-3 h-3 text-white" viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
                      </svg>
                    </div>
                  {/if}

                  {#if progressPercent(episode) > 0}
                    <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
                      <div class="h-full bg-blue-500 rounded-r-full" style="width: {progressPercent(episode)}%"></div>
                    </div>
                  {/if}
                </div>

                <!-- Episode Info -->
                <div class="flex-1 min-w-0 py-1">
                  <div class="flex items-center gap-2 mb-1">
                    <span class="text-xs text-gray-500 font-medium">
                      S{seasonNumber(item?.name)} · E{episode.index_number ?? "?"}
                    </span>
                    {#if episode.run_time_ticks}
                      <span class="text-xs text-gray-500">{formatRuntime(episode.run_time_ticks)}</span>
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
          <!-- Grid view -->
          <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-3">
            {#each episodes as episode (episode.id)}
              <button
                onclick={() => navigateToItem(episode.id)}
                class="rounded-lg overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer"
              >
                <div class="relative">
                  {#if episode.backdrop_tag}
                    <img
                      src={`${IMAGE_BASE}/backdrop/${episode.id}?tag=${episode.backdrop_tag}`}
                      alt={episode.name}
                      onload={handleImageLoad}
                      class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0"
                    />
                  {:else}
                    <div class="w-full aspect-video bg-gray-800 flex items-center justify-center">
                      <span class="text-gray-500 text-xs">E{episode.index_number ?? "?"}</span>
                    </div>
                  {/if}
                  <div class="absolute inset-0 bg-gray-800 -z-10"></div>

                  {#if episode.played}
                    <div class="absolute top-1.5 right-1.5 bg-green-500/90 rounded-full p-0.5">
                      <svg class="w-3 h-3 text-white" viewBox="0 0 20 20" fill="currentColor">
                        <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/>
                      </svg>
                    </div>
                  {/if}

                  {#if progressPercent(episode) > 0}
                    <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
                      <div class="h-full bg-blue-500 rounded-r-full" style="width: {progressPercent(episode)}%"></div>
                    </div>
                  {/if}
                </div>
                <div class="p-2">
                  <p class="text-xs text-gray-500 mb-0.5">E{episode.index_number ?? "?"}</p>
                  <p class="text-sm text-white truncate font-medium">{episode.name}</p>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      {:else}
        <p class="text-gray-500 text-sm text-center py-8">No episodes found for this season.</p>
      {/if}
    </div>
  </main>

<!-- Fallback for unknown types -->
{:else if item}
  <main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
    <div class="text-center">
      <p class="text-gray-400 text-sm mb-4">Unsupported item type: {item.type}</p>
      <button onclick={goBack} class="text-blue-400 hover:text-blue-300 text-sm">Go back</button>
    </div>
  </main>
{/if}
