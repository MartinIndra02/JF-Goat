<script lang="ts">
  import { push } from "svelte-spa-router";
  import type { MediaItem, Person } from "../../lib/types";
  import {
    seasonNumber, formatRuntime,
    progressPercent, handleImageLoad, personImageUrl,
    episodeThumbnailUrl,
  } from "./detailHelpers";

  import DetailBackdrop from "./components/DetailBackdrop.svelte";
  import DetailMetadata from "./components/DetailMetadata.svelte";
  import HorizontalCarousel from "./components/HorizontalCarousel.svelte";

  let {
    item,
    episodes = [],
    people = [],
    onTogglePlayed,
    onToggleFavorite,
  }: {
    item: MediaItem;
    episodes: MediaItem[];
    people: Person[];
    onTogglePlayed: (id: string, played: boolean) => void;
    onToggleFavorite: (id: string, isFavorite: boolean) => void;
  } = $props();

  let overviewExpanded = $state(false);
  let seasonEpisodesViewMode = $state<"list" | "grid">("list");

  function navigateToItem(id: string) { push(`/item?id=${id}`); }
  function goBack() { window.history.length > 1 ? window.history.back() : push("/home"); }
</script>

<main class="app-stage min-h-screen text-[var(--text-primary)]">
  <!-- Hero Backdrop -->
  <DetailBackdrop {item} {goBack} clampHeight="clamp(300px, 50vh, 500px)">
    {#snippet rightControls()}
      <button class="h-10 w-10 grid place-items-center bg-[rgba(10,18,31,0.64)] border border-white/22 rounded-xl backdrop-blur-xl text-[var(--text-primary)] hover:bg-[rgba(22,34,54,0.76)] transition-colors" aria-label="Download"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd"/></svg></button>
      <button class="h-10 w-10 grid place-items-center bg-[rgba(10,18,31,0.64)] border border-white/22 rounded-xl backdrop-blur-xl text-[var(--text-primary)] hover:bg-[rgba(22,34,54,0.76)] transition-colors" aria-label="Layout"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path d="M5 3a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2V5a2 2 0 00-2-2H5zM5 11a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2v-2a2 2 0 00-2-2H5zM11 5a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V5zM11 13a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z"/></svg></button>
      <button class="h-10 w-10 grid place-items-center bg-[rgba(10,18,31,0.64)] border border-white/22 rounded-xl backdrop-blur-xl text-[var(--text-primary)] hover:bg-[rgba(22,34,54,0.76)] transition-colors" aria-label="Sync"><svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg></button>
    {/snippet}
  </DetailBackdrop>

  <div class="relative -mt-28 z-10 px-5 pb-16 max-w-3xl mx-auto">
    <div class="text-center mb-5">
      {#if item.series_name && item.series_id}
        <button onclick={() => item?.series_id && navigateToItem(item.series_id)} class="text-blue-400 hover:text-blue-300 text-lg font-bold mb-1 transition-colors cursor-pointer inline-flex items-center gap-1">
          <span>{item.series_name}</span>
        </button>
      {/if}
      <h1 class="text-2xl font-bold text-white mb-3">{item.name}</h1>

      <!-- Metadata block component -->
      <DetailMetadata {item} />
    </div>

    <!-- Actions row -->
    <div class="flex flex-wrap items-center gap-3 mb-5">
      <div class="flex items-center gap-2">
        <button onclick={() => onToggleFavorite(item.id, item.is_favorite)} class="flex items-center gap-1.5 px-4 py-2 rounded-xl bg-white/5 hover:bg-white/12 border border-white/10 transition-all {item.is_favorite ? 'text-rose-400 hover:text-rose-300 bg-rose-500/10 border-rose-500/20' : 'text-gray-400 hover:text-white'}">
          <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/></svg>
          <span>Favorite</span>
        </button>
        <button onclick={() => onTogglePlayed(item.id, item.played)} class="flex items-center gap-1.5 px-4 py-2 rounded-xl bg-white/5 hover:bg-white/12 border border-white/10 transition-all {item.played ? 'text-emerald-400 hover:text-emerald-300 bg-emerald-500/10 border-emerald-500/20' : 'text-gray-400 hover:text-white'}">
          <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg>
          <span>Watched</span>
        </button>
      </div>

      <!-- View toggle -->
      <div class="flex items-center ml-auto bg-white/5 rounded-xl overflow-hidden border border-white/10">
        <button onclick={() => seasonEpisodesViewMode = "list"} class="px-3.5 py-1.5 text-xs font-semibold transition-colors flex items-center gap-1.5 {seasonEpisodesViewMode === 'list' ? 'bg-white/12 text-white' : 'text-gray-400 hover:text-white hover:bg-white/5'}">
          <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1z" clip-rule="evenodd"/></svg>
          List
        </button>
        <button onclick={() => seasonEpisodesViewMode = "grid"} class="px-3.5 py-1.5 text-xs font-semibold transition-colors flex items-center gap-1.5 {seasonEpisodesViewMode === 'grid' ? 'bg-white/12 text-white' : 'text-gray-400 hover:text-white hover:bg-white/5'}">
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
      <HorizontalCarousel
        title="Cast & Crew"
        items={people}
        getKey={(person, i) => person.id + '-' + i}
      >
        {#snippet renderCard(person)}
          <div class="flex-shrink-0 w-[80px] text-center">
            <div class="relative w-[72px] h-[72px] mx-auto rounded-lg overflow-hidden bg-gray-800 mb-1.5 ring-1 ring-white/10">
              {#if person.image_tag}<img src={personImageUrl(person.id, person.image_tag)} alt={person.name} onload={handleImageLoad} class="w-full h-full object-cover transition-opacity duration-300 opacity-0" />
              {:else}<div class="w-full h-full flex items-center justify-center"><svg class="w-7 h-7 text-gray-600" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 9a3 3 0 100-6 3 3 0 000 6zm-7 9a7 7 0 1114 0H3z" clip-rule="evenodd"/></svg></div>{/if}
            </div>
            <p class="text-[11px] text-white font-medium truncate leading-tight">{person.name}</p>
            {#if person.role}<p class="text-[10px] text-gray-500 truncate leading-tight">{person.role}</p>
            {:else if person.person_type}<p class="text-[10px] text-gray-500 truncate leading-tight">{person.person_type}</p>{/if}
          </div>
        {/snippet}
      </HorizontalCarousel>
    {/if}
  </div>
</main>
