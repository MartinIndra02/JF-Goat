<script lang="ts">
  import { push } from "svelte-spa-router";
  import type { MediaItem, Person, MediaStreamInfo, ExternalUrl } from "../../lib/types";
  import {
    IMAGE_BASE, seasonNumber, formatRuntime,
    progressPercent, handleImageLoad, backdropUrl, personImageUrl, scrollCarousel,
    episodeThumbnailUrl,
  } from "./detailHelpers";

  let {
    item,
    seasons = [],
    episodes = [],
    allSeasonEpisodes = {},
    selectedSeasonId = null,
    people = [],
    similarItems = [],
    mediaStreams = null,
    externalUrls = [],
    episodesLoading = false,
    onLoadSeasonEpisodes,
    onTogglePlayed,
    onToggleFavorite,
  }: {
    item: MediaItem;
    seasons: MediaItem[];
    episodes: MediaItem[];
    allSeasonEpisodes: Record<string, MediaItem[]>;
    selectedSeasonId: string | null;
    people: Person[];
    similarItems: MediaItem[];
    mediaStreams: MediaStreamInfo | null;
    externalUrls: ExternalUrl[];
    episodesLoading: boolean;
    onLoadSeasonEpisodes: (seasonId: string) => void;
    onTogglePlayed: (id: string, played: boolean) => void;
    onToggleFavorite: (id: string, isFavorite: boolean) => void;
  } = $props();

  let overviewExpanded = $state(false);
  let contextMenuOpen = $state(false);
  let audioDropdownOpen = $state(false);
  let subtitleDropdownOpen = $state(false);
  let selectedAudioIndex = $state<number | null>(null);
  let selectedSubtitleIndex = $state<number | null>(null);
  let episodeScrollEl = $state<HTMLElement | null>(null);
  let seasonScrollEl = $state<HTMLElement | null>(null);
  let castScrollEl = $state<HTMLElement | null>(null);
  let relatedScrollEl = $state<HTMLElement | null>(null);

  // Resume episode: find in-progress or first unwatched
  const resumeEpisode = $derived.by(() => {
    for (const eps of Object.values(allSeasonEpisodes)) {
      for (const ep of eps) {
        if (ep.playback_ticks > 0 && !ep.played) return ep;
      }
    }
    for (const eps of Object.values(allSeasonEpisodes)) {
      for (const ep of eps) {
        if (!ep.played) return ep;
      }
    }
    return null;
  });

  $effect(() => {
    if (mediaStreams) {
      const defAudio = mediaStreams.audio.find(a => a.is_default);
      if (defAudio && selectedAudioIndex === null) selectedAudioIndex = defAudio.index;
      const defSub = mediaStreams.subtitle.find(s => s.is_default);
      if (defSub && selectedSubtitleIndex === null) selectedSubtitleIndex = defSub.index;
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

  function navigateToItem(id: string) { push(`/item?id=${id}`); }
  function goBack() { window.history.length > 1 ? window.history.back() : push("/home"); }
  function closeContextMenu() { contextMenuOpen = false; }
</script>

<main class="min-h-screen bg-gray-900 text-white">
  <!-- Hero Backdrop -->
  <div class="relative w-full overflow-hidden" style="height: clamp(340px, 55vh, 560px);">
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

  <div class="relative -mt-32 z-10 px-5 pb-16 max-w-3xl mx-auto">
    <div class="text-center mb-6">
      <h1 class="text-3xl sm:text-4xl font-bold text-white leading-tight mb-3">{item.name}</h1>

      <!-- Metadata -->
      <div class="flex flex-wrap items-center justify-center gap-2 mb-3">
        {#if item.official_rating}
          <span class="inline-flex items-center gap-1 text-xs font-semibold text-gray-200 bg-white/10 px-2.5 py-1 rounded-md border border-white/15">{item.official_rating}</span>
          <span class="w-1 h-1 rounded-full bg-gray-500"></span>
        {/if}
        {#if item.production_year}
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
      {#if resumeEpisode}
        <button onclick={() => resumeEpisode && navigateToItem(resumeEpisode.id)} class="relative w-full flex items-center justify-center gap-2.5 py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-semibold text-sm transition-colors overflow-hidden">
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
          <span>{progressPercent(resumeEpisode) > 0 ? "Resume" : "Play"} S{seasonNumber(resumeEpisode.season_name)} - E{resumeEpisode.index_number ?? "?"}</span>
          {#if progressPercent(resumeEpisode) > 0}
            <div class="absolute bottom-0 left-0 right-0 h-1 bg-blue-900"><div class="h-full bg-blue-300 transition-all" style="width: {progressPercent(resumeEpisode)}%"></div></div>
          {/if}
        </button>
      {:else}
        <button class="w-full flex items-center justify-center gap-2.5 py-3 bg-blue-600 hover:bg-blue-700 rounded-xl font-semibold text-sm transition-colors">
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
          <span>Play</span>
        </button>
      {/if}
    </div>

    {#if resumeEpisode && progressPercent(resumeEpisode) > 0}
      <div class="mb-3">
        <button class="w-full flex items-center justify-center gap-2 py-2 bg-white/10 hover:bg-white/15 rounded-xl text-sm text-gray-300 transition-colors">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg>
          Play from start
        </button>
      </div>
    {/if}

    <!-- Media spec badges -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    {#if mediaStreams}
      <div class="flex flex-wrap items-center gap-2 mb-3 justify-center">
        {#if mediaStreams.video_label}
          <span class="inline-flex items-center gap-1.5 text-xs font-medium text-blue-300 bg-blue-500/15 px-3 py-1.5 rounded-lg border border-blue-500/30">
            <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path d="M4 3a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V5a2 2 0 00-2-2H4zm12 12H4l4-8 3 6 2-4 3 6z"/></svg>
            {mediaStreams.video_label}
          </span>
        {/if}
        {#if mediaStreams.audio.length > 0}
          <div class="relative">
            <button onclick={() => { subtitleDropdownOpen = false; audioDropdownOpen = !audioDropdownOpen; }} class="inline-flex items-center gap-1.5 text-xs font-medium text-blue-300 bg-blue-500/15 px-3 py-1.5 rounded-lg border border-blue-500/30 hover:bg-blue-500/25 transition-colors cursor-pointer">
              <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M9.383 3.076A1 1 0 0110 4v12a1 1 0 01-1.707.707L4.586 13H2a1 1 0 01-1-1V8a1 1 0 011-1h2.586l3.707-3.707a1 1 0 011.09-.217zM14.657 2.929a1 1 0 011.414 0A9.972 9.972 0 0119 10a9.972 9.972 0 01-2.929 7.071a1 1 0 01-1.414-1.414A7.971 7.971 0 0017 10c0-2.21-.894-4.208-2.343-5.657a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
              {selectedAudioLabel()}
              <svg class="w-3 h-3" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
            </button>
            {#if audioDropdownOpen}
              <div onclick={() => audioDropdownOpen = false} class="fixed inset-0 z-40"></div>
              <div class="absolute left-0 top-full mt-1 w-56 bg-gray-800 border border-white/10 rounded-xl shadow-xl z-50 py-1.5 max-h-64 overflow-y-auto">
                {#each mediaStreams.audio as track}
                  <button onclick={() => { selectedAudioIndex = track.index; audioDropdownOpen = false; }} class="w-full text-left px-4 py-2 text-sm transition-colors {track.index === selectedAudioIndex ? 'text-blue-400 bg-blue-500/10' : 'text-gray-200 hover:bg-white/10'}">{track.display_title}</button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
        {#if mediaStreams.subtitle.length > 0}
          <div class="relative">
            <button onclick={() => { audioDropdownOpen = false; subtitleDropdownOpen = !subtitleDropdownOpen; }} class="inline-flex items-center gap-1.5 text-xs font-medium text-blue-300 bg-blue-500/15 px-3 py-1.5 rounded-lg border border-blue-500/30 hover:bg-blue-500/25 transition-colors cursor-pointer">
              <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M4 3a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V5a2 2 0 00-2-2H4zm2 3a1 1 0 000 2h3a1 1 0 000-2H6zm0 4a1 1 0 000 2h8a1 1 0 100-2H6zm0 4a1 1 0 100 2h5a1 1 0 100-2H6z" clip-rule="evenodd"/></svg>
              {selectedSubtitleLabel() ?? "Subtitles"}
              <svg class="w-3 h-3" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
            </button>
            {#if subtitleDropdownOpen}
              <div onclick={() => subtitleDropdownOpen = false} class="fixed inset-0 z-40"></div>
              <div class="absolute left-0 top-full mt-1 w-56 bg-gray-800 border border-white/10 rounded-xl shadow-xl z-50 py-1.5 max-h-64 overflow-y-auto">
                <button onclick={() => { selectedSubtitleIndex = null; subtitleDropdownOpen = false; }} class="w-full text-left px-4 py-2 text-sm transition-colors {selectedSubtitleIndex === null ? 'text-blue-400 bg-blue-500/10' : 'text-gray-200 hover:bg-white/10'}">None</button>
                {#each mediaStreams.subtitle as track}
                  <button onclick={() => { selectedSubtitleIndex = track.index; subtitleDropdownOpen = false; }} class="w-full text-left px-4 py-2 text-sm transition-colors {track.index === selectedSubtitleIndex ? 'text-blue-400 bg-blue-500/10' : 'text-gray-200 hover:bg-white/10'}">{track.display_title}</button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    <div class="flex justify-center my-2"><div class="w-3 h-0.5 rounded-full bg-white/20"></div></div>

    <!-- Action buttons -->
    <div class="flex items-center justify-center gap-2 mb-5">
      <button aria-label="Toggle favorite" onclick={() => onToggleFavorite(item.id, item.is_favorite)} class="p-2.5 rounded-full hover:bg-white/10 transition-colors {item.is_favorite ? 'text-red-400' : 'text-gray-400'}"><svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/></svg></button>
      <button aria-label="Toggle watched" onclick={() => onTogglePlayed(item.id, item.played)} class="p-2.5 rounded-full hover:bg-white/10 transition-colors {item.played ? 'text-green-400' : 'text-gray-400'}"><svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></button>
      <div class="relative">
        <button aria-label="More options" onclick={() => contextMenuOpen = !contextMenuOpen} class="p-2.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path d="M6 10a2 2 0 11-4 0 2 2 0 014 0zM12 10a2 2 0 11-4 0 2 2 0 014 0zM16 12a2 2 0 100-4 2 2 0 000 4z"/></svg></button>
        {#if contextMenuOpen}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div onclick={closeContextMenu} class="fixed inset-0 z-40"></div>
          <div class="absolute right-0 top-full mt-1 w-52 bg-gray-800 border border-white/10 rounded-xl shadow-xl z-50 py-1.5">
            <button onclick={() => { onTogglePlayed(item.id, item.played); closeContextMenu(); }} class="w-full text-left px-4 py-2.5 text-sm text-gray-200 hover:bg-white/10 transition-colors flex items-center gap-2.5"><svg class="w-4 h-4 text-gray-400" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg>{item.played ? "Mark as unwatched" : "Mark as watched"}</button>
            <button onclick={() => { onToggleFavorite(item.id, item.is_favorite); closeContextMenu(); }} class="w-full text-left px-4 py-2.5 text-sm text-gray-200 hover:bg-white/10 transition-colors flex items-center gap-2.5"><svg class="w-4 h-4 text-gray-400" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/></svg>{item.is_favorite ? "Remove from favorites" : "Add to favorites"}</button>
            <button onclick={closeContextMenu} class="w-full text-left px-4 py-2.5 text-sm text-gray-200 hover:bg-white/10 transition-colors flex items-center gap-2.5"><svg class="w-4 h-4 text-gray-400" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-11a1 1 0 10-2 0v2H7a1 1 0 100 2h2v2a1 1 0 102 0v-2h2a1 1 0 100-2h-2V7z" clip-rule="evenodd"/></svg>Add to playlist</button>
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

    <!-- Episodes section with season dropdown -->
    {#if seasons.length > 0}
      <div class="mb-8">
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-base font-semibold text-white">Episodes</h2>
          <div class="relative">
            <select
              onchange={(e) => { const target = e.target as HTMLSelectElement; onLoadSeasonEpisodes(target.value); }}
              value={selectedSeasonId ?? ""}
              class="appearance-none bg-white/10 text-white text-sm px-3 py-1.5 pr-8 rounded-lg border border-white/10 focus:outline-none focus:ring-2 focus:ring-blue-500 cursor-pointer"
            >
              {#each seasons as season (season.id)}
                <option value={season.id} class="bg-gray-800">{season.name}</option>
              {/each}
            </select>
            <svg class="absolute right-2 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 pointer-events-none" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
          </div>
        </div>

        {#if episodesLoading}
          <div class="flex items-center justify-center h-24"><svg class="w-6 h-6 text-blue-400 animate-spin" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25"/><path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round"/></svg></div>
        {:else if episodes.length > 0}
          <div class="relative">
            <div class="flex items-center justify-end gap-1 mb-2">
              <button onclick={() => scrollCarousel(episodeScrollEl, 'left')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></button>
              <button onclick={() => scrollCarousel(episodeScrollEl, 'right')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd"/></svg></button>
            </div>
            <div bind:this={episodeScrollEl} class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-5 px-5">
              {#each episodes as episode (episode.id)}
                <button onclick={() => navigateToItem(episode.id)} class="flex-shrink-0 w-48 rounded-xl overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer">
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
                  <div class="p-2.5">
                    <p class="text-[11px] text-gray-500 mb-0.5">S{seasonNumber(episode.season_name)} - E{episode.index_number ?? "?"}{#if episode.run_time_ticks}<span class="ml-1">· {formatRuntime(episode.run_time_ticks)}</span>{/if}</p>
                    <p class="text-sm text-white truncate font-medium">{episode.name}</p>
                  </div>
                </button>
              {/each}
            </div>
          </div>
        {:else}
          <p class="text-gray-500 text-sm text-center py-6">No episodes found for this season.</p>
        {/if}
      </div>
    {/if}

    <!-- Seasons carousel -->
    {#if seasons.length > 1}
      <div class="mb-8">
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-base font-semibold text-white">Seasons</h2>
          <div class="flex items-center gap-1">
            <button onclick={() => scrollCarousel(seasonScrollEl, 'left')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></button>
            <button onclick={() => scrollCarousel(seasonScrollEl, 'right')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd"/></svg></button>
          </div>
        </div>
        <div bind:this={seasonScrollEl} class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-5 px-5">
          {#each seasons as season (season.id)}
            <button onclick={() => navigateToItem(season.id)} class="flex-shrink-0 w-28 rounded-xl overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer group">
              <div class="relative">
                {#if season.image_tag}
                  <img src={`${IMAGE_BASE}/poster/${season.id}?tag=${season.image_tag}`} alt={season.name} onload={handleImageLoad} class="w-full aspect-[2/3] object-cover transition-opacity duration-300 opacity-0" />
                {:else}
                  <div class="w-full aspect-[2/3] bg-gray-800 flex items-center justify-center"><span class="text-gray-500 text-xs text-center px-2">{season.name}</span></div>
                {/if}
                <div class="absolute inset-0 bg-gray-800 -z-10"></div>
                {#if season.played}<div class="absolute top-1.5 right-1.5 bg-green-500/90 rounded-full p-0.5"><svg class="w-3 h-3 text-white" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></div>{/if}
              </div>
              <div class="p-2"><p class="text-xs text-white truncate font-medium">{season.name}</p></div>
            </button>
          {/each}
        </div>
      </div>
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

    <!-- Related -->
    {#if similarItems.length > 0}
      <div class="mb-8">
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-base font-semibold text-white">Related</h2>
          <div class="flex items-center gap-1">
            <button onclick={() => scrollCarousel(relatedScrollEl, 'left')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></button>
            <button onclick={() => scrollCarousel(relatedScrollEl, 'right')} class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd"/></svg></button>
          </div>
        </div>
        <div bind:this={relatedScrollEl} class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-5 px-5">
          {#each similarItems as related (related.id)}
            <button onclick={() => navigateToItem(related.id)} class="flex-shrink-0 w-28 rounded-xl overflow-hidden bg-white/5 hover:bg-white/10 transition-colors text-left cursor-pointer group">
              <div class="relative">
                {#if related.image_tag}<img src={`${IMAGE_BASE}/poster/${related.id}?tag=${related.image_tag}`} alt={related.name} onload={handleImageLoad} class="w-full aspect-[2/3] object-cover transition-opacity duration-300 opacity-0" />
                {:else}<div class="w-full aspect-[2/3] bg-gray-800 flex items-center justify-center"><span class="text-gray-500 text-xs text-center px-2">{related.name}</span></div>{/if}
                <div class="absolute inset-0 bg-gray-800 -z-10"></div>
              </div>
              <div class="p-2">
                <p class="text-xs text-white truncate font-medium">{related.name}</p>
                {#if related.production_year}<p class="text-[11px] text-gray-500">{related.production_year}</p>{/if}
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
