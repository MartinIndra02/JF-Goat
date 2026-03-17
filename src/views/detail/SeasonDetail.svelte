<script lang="ts">
  import { push } from "svelte-spa-router";
  import type { MediaItem, Person } from "../../lib/types";
  import {
    IMAGE_BASE, seasonNumber, formatRuntime,
    progressPercent, handleImageLoad, backdropUrl, personImageUrl, scrollCarousel,
    episodeThumbnailUrl,
  } from "./detailHelpers";

  let {
    item,
    episodes = [],
    seasons = [],
    people = [],
    onTogglePlayed,
    onToggleFavorite,
  }: {
    item: MediaItem;
    episodes: MediaItem[];
    seasons: MediaItem[];
    people: Person[];
    onTogglePlayed: (id: string, played: boolean) => void;
    onToggleFavorite: (id: string, isFavorite: boolean) => void;
  } = $props();

  let overviewExpanded = $state(false);
  let seasonEpisodesViewMode = $state<"list" | "grid">("list");
  let castScrollEl = $state<HTMLElement | null>(null);

  function navigateToItem(id: string) { push(`/item?id=${id}`); }
  function goBack() { window.history.length > 1 ? window.history.back() : push("/home"); }
</script>

<main class="min-h-screen bg-gray-900 text-white">
  <!-- Hero Backdrop -->
  <div class="relative w-full overflow-hidden" style="height: clamp(300px, 50vh, 500px);">
    {#if backdropUrl(item)}
      <img src={backdropUrl(item)} alt="" onload={handleImageLoad} class="absolute inset-0 w-full h-full object-cover object-top transition-opacity duration-500 opacity-0" />
    {/if}
    <div class="absolute inset-0 bg-gray-800 -z-10"></div>
    <div class="absolute inset-0 bg-gradient-to-t from-gray-900 via-gray-900/40 to-transparent"></div>

    <button aria-label="Go back" onclick={goBack} class="absolute top-4 left-4 z-10 p-2 bg-black/40 hover:bg-black/60 rounded-full backdrop-blur-sm transition-colors">
      <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/></svg>
    </button>

    <div class="absolute top-4 right-4 z-10 flex items-center gap-1.5">
      <button class="p-2 bg-black/40 hover:bg-black/60 rounded-full backdrop-blur-sm transition-colors text-gray-300" aria-label="Download"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd"/></svg></button>
      <button class="p-2 bg-black/40 hover:bg-black/60 rounded-full backdrop-blur-sm transition-colors text-gray-300" aria-label="Layout"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path d="M5 3a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2V5a2 2 0 00-2-2H5zM5 11a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2v-2a2 2 0 00-2-2H5zM11 5a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V5zM11 13a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z"/></svg></button>
      <button class="p-2 bg-black/40 hover:bg-black/60 rounded-full backdrop-blur-sm transition-colors text-gray-300" aria-label="Sync"><svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg></button>
    </div>
  </div>

  <div class="relative -mt-28 z-10 px-5 pb-16 max-w-3xl mx-auto">
    <div class="text-center mb-5">
      {#if item.series_name && item.series_id}
        <button onclick={() => item?.series_id && navigateToItem(item.series_id)} class="text-blue-400 hover:text-blue-300 text-lg font-bold mb-1 transition-colors cursor-pointer inline-flex items-center gap-1">
          <span>{item.series_name}</span>
        </button>
      {/if}
      <h1 class="text-2xl font-bold text-white mb-3">{item.name}</h1>

      <div class="flex flex-wrap items-center justify-center gap-2 mb-3">
        {#if item.production_year}
          <span class="inline-flex items-center gap-1.5 text-xs text-gray-300 bg-white/8 px-2.5 py-1 rounded-md">
            <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clip-rule="evenodd"/></svg>
            {item.production_year}
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
    </div>

    <!-- Actions row -->
    <div class="flex flex-wrap items-center gap-3 mb-5">
      <div class="flex items-center gap-2">
        <button onclick={() => onToggleFavorite(item.id, item.is_favorite)} class="flex items-center gap-1.5 px-3.5 py-2 rounded-lg bg-white/10 hover:bg-white/15 transition-colors text-sm {item.is_favorite ? 'text-red-400' : 'text-gray-300'}">
          <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/></svg>
          <span>Favorite</span>
        </button>
        <button onclick={() => onTogglePlayed(item.id, item.played)} class="flex items-center gap-1.5 px-3.5 py-2 rounded-lg bg-white/10 hover:bg-white/15 transition-colors text-sm {item.played ? 'text-green-400' : 'text-gray-300'}">
          <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg>
          <span>Watched</span>
        </button>
      </div>

      <!-- View toggle -->
      <div class="flex items-center ml-auto bg-white/5 rounded-lg overflow-hidden border border-white/10">
        <button onclick={() => seasonEpisodesViewMode = "list"} class="px-3 py-1.5 text-xs font-medium transition-colors flex items-center gap-1.5 {seasonEpisodesViewMode === 'list' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-white'}">
          <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1z" clip-rule="evenodd"/></svg>
          List
        </button>
        <button onclick={() => seasonEpisodesViewMode = "grid"} class="px-3 py-1.5 text-xs font-medium transition-colors flex items-center gap-1.5 {seasonEpisodesViewMode === 'grid' ? 'bg-blue-600 text-white' : 'text-gray-400 hover:text-white'}">
          <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path d="M5 3a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2V5a2 2 0 00-2-2H5zM5 11a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2v-2a2 2 0 00-2-2H5zM11 5a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V5zM11 13a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z"/></svg>
          Grid
        </button>
      </div>
    </div>

    <!-- Season overview -->
    {#if item.overview}
      <div class="mb-5">
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

    <!-- Episodes list or grid -->
    {#if episodes.length > 0}
      {#if seasonEpisodesViewMode === "list"}
        <div class="space-y-2 mb-8">
          {#each episodes as episode (episode.id)}
            <button onclick={() => navigateToItem(episode.id)} class="w-full flex gap-3.5 p-2.5 rounded-xl bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer">
              <div class="flex-shrink-0 w-36 sm:w-40 relative overflow-hidden rounded-lg">
                {#if episodeThumbnailUrl(episode)}
                  <img src={episodeThumbnailUrl(episode)} alt={episode.name} onload={handleImageLoad} class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0" />
                {:else}
                  <div class="w-full aspect-video bg-gray-800 flex items-center justify-center"><span class="text-gray-500 text-xs">E{episode.index_number ?? "?"}</span></div>
                {/if}
                <div class="absolute inset-0 bg-gray-800 -z-10"></div>
                {#if episode.played}<div class="absolute top-1 right-1 bg-green-500/90 rounded-full p-0.5"><svg class="w-3 h-3 text-white" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></div>{/if}
                {#if progressPercent(episode) > 0}<div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50"><div class="h-full bg-blue-500 rounded-r-full" style="width: {progressPercent(episode)}%"></div></div>{/if}
              </div>
              <div class="flex-1 min-w-0 py-0.5">
                <div class="flex items-center gap-2 mb-0.5">
                  <span class="text-[11px] text-gray-500 font-medium">S{seasonNumber(item.season_name ?? item.name)} - E{episode.index_number ?? "?"}</span>
                  {#if episode.run_time_ticks}<span class="text-[11px] text-gray-500">· {formatRuntime(episode.run_time_ticks)}</span>{/if}
                </div>
                <p class="text-sm text-white font-medium truncate">{episode.name}</p>
                {#if episode.overview}<p class="text-xs text-gray-400 line-clamp-2 mt-1">{episode.overview}</p>{/if}
              </div>
            </button>
          {/each}
        </div>
      {:else}
        <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-3 mb-8">
          {#each episodes as episode (episode.id)}
            <button onclick={() => navigateToItem(episode.id)} class="rounded-xl overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer">
              <div class="relative">
                {#if episodeThumbnailUrl(episode)}
                  <img src={episodeThumbnailUrl(episode)} alt={episode.name} onload={handleImageLoad} class="w-full aspect-video object-cover transition-opacity duration-300 opacity-0" />
                {:else}
                  <div class="w-full aspect-video bg-gray-800 flex items-center justify-center"><span class="text-gray-500 text-xs">E{episode.index_number ?? "?"}</span></div>
                {/if}
                <div class="absolute inset-0 bg-gray-800 -z-10"></div>
                {#if episode.played}<div class="absolute top-1.5 right-1.5 bg-green-500/90 rounded-full p-0.5"><svg class="w-3 h-3 text-white" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></div>{/if}
                {#if progressPercent(episode) > 0}<div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50"><div class="h-full bg-blue-500 rounded-r-full" style="width: {progressPercent(episode)}%"></div></div>{/if}
              </div>
              <div class="p-2">
                <p class="text-[11px] text-gray-500 mb-0.5">E{episode.index_number ?? "?"}</p>
                <p class="text-sm text-white truncate font-medium">{episode.name}</p>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    {:else}
      <p class="text-gray-500 text-sm text-center py-8">No episodes found for this season.</p>
    {/if}

    <!-- Cast & Crew -->
    {#if people.length > 0}
      <div class="mb-8">
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-base font-semibold text-white">Cast & Crew</h2>
          <div class="flex items-center gap-1">
            <button onclick={() => scrollCarousel(castScrollEl, 'left')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></button>
            <button onclick={() => scrollCarousel(castScrollEl, 'right')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd"/></svg></button>
          </div>
        </div>
        <div bind:this={castScrollEl} class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-5 px-5">
          {#each people as person, i (person.id + '-' + i)}
            <div class="flex-shrink-0 w-[80px] text-center">
              <div class="relative w-[72px] h-[72px] mx-auto rounded-lg overflow-hidden bg-gray-800 mb-1.5 ring-1 ring-white/10">
                {#if person.image_tag}<img src={personImageUrl(person.id, person.image_tag)} alt={person.name} onload={handleImageLoad} class="w-full h-full object-cover transition-opacity duration-300 opacity-0" />
                {:else}<div class="w-full h-full flex items-center justify-center"><svg class="w-7 h-7 text-gray-600" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clip-rule="evenodd"/></svg></div>{/if}
              </div>
              <p class="text-[11px] text-white font-medium truncate leading-tight">{person.name}</p>
              {#if person.role}<p class="text-[10px] text-gray-500 truncate leading-tight">{person.role}</p>
              {:else if person.person_type}<p class="text-[10px] text-gray-500 truncate leading-tight">{person.person_type}</p>{/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</main>
