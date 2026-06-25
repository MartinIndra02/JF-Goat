<script lang="ts">
  import { push } from "svelte-spa-router";
  import type { MediaItem, Person, MediaStreamInfo, ExternalUrl } from "../../lib/types";
  import {
    IMAGE_BASE, seasonNumber, formatRuntime,
    progressPercent, handleImageLoad, personImageUrl,
    episodeThumbnailUrl,
  } from "./detailHelpers";

  import DetailBackdrop from "./components/DetailBackdrop.svelte";
  import DetailMetadata from "./components/DetailMetadata.svelte";
  import HorizontalCarousel from "./components/HorizontalCarousel.svelte";

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

<main class="app-stage min-h-screen text-[var(--text-primary)]">
  <!-- Hero Backdrop -->
  <DetailBackdrop {item} {goBack}>
    {#snippet rightControls()}
      <button class="h-10 w-10 grid place-items-center bg-[rgba(10,18,31,0.64)] border border-white/22 rounded-xl backdrop-blur-xl text-[var(--text-primary)] hover:bg-[rgba(22,34,54,0.76)] transition-colors" aria-label="Download"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd"/></svg></button>
      <button class="h-10 w-10 grid place-items-center bg-[rgba(10,18,31,0.64)] border border-white/22 rounded-xl backdrop-blur-xl text-[var(--text-primary)] hover:bg-[rgba(22,34,54,0.76)] transition-colors" aria-label="Layout"><svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path d="M5 3a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2V5a2 2 0 00-2-2H5zM5 11a2 2 0 00-2 2v2a2 2 0 002 2h2a2 2 0 002-2v-2a2 2 0 00-2-2H5zM11 5a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V5zM11 13a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z"/></svg></button>
      <button class="h-10 w-10 grid place-items-center bg-[rgba(10,18,31,0.64)] border border-white/22 rounded-xl backdrop-blur-xl text-[var(--text-primary)] hover:bg-[rgba(22,34,54,0.76)] transition-colors" aria-label="Sync"><svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg></button>
    {/snippet}
  </DetailBackdrop>

  <div class="relative -mt-32 z-10 px-5 pb-16 max-w-3xl mx-auto">
    <div class="text-center mb-6">
      <h1 class="text-3xl sm:text-4xl font-bold text-white leading-tight mb-3">{item.name}</h1>

      <!-- Metadata block component -->
      <DetailMetadata {item} />
    </div>

    <!-- Play / Resume button -->
    <div class="mb-2">
      {#if resumeEpisode}
        <button onclick={() => resumeEpisode && navigateToItem(resumeEpisode.id)} class="relative w-full flex items-center justify-center gap-2.5 py-3.5 bg-gradient-to-br from-cyan-200 via-sky-300 to-amber-300 text-slate-950 hover:brightness-110 rounded-xl font-bold text-sm shadow-[0_10px_26px_rgba(102,216,255,0.25)] hover:scale-[1.01] active:scale-[0.99] transition-all overflow-hidden">
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
          <span>{progressPercent(resumeEpisode) > 0 ? "Resume" : "Play"} S{seasonNumber(resumeEpisode.season_name)} - E{resumeEpisode.index_number ?? "?"}</span>
          {#if progressPercent(resumeEpisode) > 0}
            <div class="absolute bottom-0 left-0 right-0 h-1 bg-slate-900/20"><div class="h-full bg-slate-950/60 transition-all" style="width: {progressPercent(resumeEpisode)}%"></div></div>
          {/if}
        </button>
      {:else}
        <button onclick={() => { const first = Object.values(allSeasonEpisodes).flat()[0]; if (first) navigateToItem(first.id); }} class="w-full flex items-center justify-center gap-2.5 py-3.5 bg-gradient-to-br from-cyan-200 via-sky-300 to-amber-300 text-slate-950 hover:brightness-110 rounded-xl font-bold text-sm shadow-[0_10px_26px_rgba(102,216,255,0.25)] hover:scale-[1.01] active:scale-[0.99] transition-all">
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
          <span>Play</span>
        </button>
      {/if}
    </div>

    {#if resumeEpisode && progressPercent(resumeEpisode) > 0}
      <div class="mb-3">
        <button onclick={() => resumeEpisode && navigateToItem(resumeEpisode.id)} class="w-full flex items-center justify-center gap-2 py-2.5 bg-white/8 hover:bg-white/14 border border-white/10 rounded-xl text-sm font-medium text-[var(--text-secondary)] hover:text-white transition-colors">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/></svg>
          Play from start
        </button>
      </div>
    {/if}

    <!-- Media spec badges -->
    {#if mediaStreams}
      <div class="flex flex-wrap items-center gap-2 mb-3 justify-center">
        {#if mediaStreams.audio.length > 0}
          <div class="relative">
            <button aria-label="Choose audio track" aria-haspopup="listbox" aria-expanded={audioDropdownOpen} onclick={() => { subtitleDropdownOpen = false; audioDropdownOpen = !audioDropdownOpen; }} class="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--text-primary)] bg-[rgba(10,18,31,0.64)] border border-white/22 px-3 py-2 rounded-xl hover:bg-[rgba(22,34,54,0.76)] transition-colors cursor-pointer">
              <svg class="w-3.5 h-3.5 text-cyan-200" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M9.383 3.076A1 1 0 0110 4v12a1 1 0 01-1.707.707L4.586 13H2a1 1 0 01-1-1V8a1 1 0 011-1h2.586l3.707-3.707a1 1 0 011.09-.217zM14.657 2.929a1 1 0 011.414 0A9.972 9.972 0 0119 10a9.972 9.972 0 01-2.929 7.071a1 1 0 01-1.414-1.414A7.971 7.971 0 0017 10c0-2.21-.894-4.208-2.343-5.657a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
              {selectedAudioLabel()}
              <svg class="w-3 h-3 text-cyan-200" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
            </button>
            {#if audioDropdownOpen}
              <button type="button" aria-label="Close audio track menu" onclick={() => audioDropdownOpen = false} class="fixed inset-0 z-40"></button>
              <div role="listbox" aria-label="Audio tracks" class="absolute left-1/2 -translate-x-1/2 top-full mt-1.5 w-64 bg-[rgba(7,14,24,0.92)] border border-white/22 rounded-xl shadow-xl z-50 p-1.5 max-h-64 overflow-y-auto backdrop-blur-2xl">
                {#each mediaStreams.audio as track}
                  <button onclick={() => { selectedAudioIndex = track.index; audioDropdownOpen = false; }} class="w-full text-left px-3 py-2 rounded-lg text-xs transition-colors text-gray-100 hover:bg-white/14 flex items-center gap-2 {track.index === selectedAudioIndex ? 'bg-cyan-400 bg-opacity-20 text-cyan-200' : ''}">{track.display_title}</button>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
        {#if mediaStreams.subtitle.length > 0}
          <div class="relative">
            <button aria-label="Choose subtitle track" aria-haspopup="listbox" aria-expanded={subtitleDropdownOpen} onclick={() => { audioDropdownOpen = false; subtitleDropdownOpen = !subtitleDropdownOpen; }} class="inline-flex items-center gap-1.5 text-xs font-semibold text-[var(--text-primary)] bg-[rgba(10,18,31,0.64)] border border-white/22 px-3 py-2 rounded-xl hover:bg-[rgba(22,34,54,0.76)] transition-colors cursor-pointer">
              <svg class="w-3.5 h-3.5 text-emerald-200" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M4 3a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V5a2 2 0 00-2-2H4zm2 3a1 1 0 000 2h3a1 1 0 000-2H6zm0 4a1 1 0 000 2h8a1 1 0 100-2H6zm0 4a1 1 0 100 2h5a1 1 0 100-2H6z" clip-rule="evenodd"/></svg>
              {selectedSubtitleLabel() ?? "Subtitles"}
              <svg class="w-3 h-3 text-emerald-200" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
            </button>
            {#if subtitleDropdownOpen}
              <button type="button" aria-label="Close subtitle track menu" onclick={() => subtitleDropdownOpen = false} class="fixed inset-0 z-40"></button>
              <div role="listbox" aria-label="Subtitle tracks" class="absolute left-1/2 -translate-x-1/2 top-full mt-1.5 w-64 bg-[rgba(7,14,24,0.92)] border border-white/22 rounded-xl shadow-xl z-50 p-1.5 max-h-64 overflow-y-auto backdrop-blur-2xl">
                <button onclick={() => { selectedSubtitleIndex = null; subtitleDropdownOpen = false; }} class="w-full text-left px-3 py-2 rounded-lg text-xs transition-colors text-gray-100 hover:bg-white/14 {selectedSubtitleIndex === null ? 'bg-cyan-400 bg-opacity-20 text-cyan-200' : ''}">None</button>
                {#each mediaStreams.subtitle as track}
                  <button onclick={() => { selectedSubtitleIndex = track.index; subtitleDropdownOpen = false; }} class="w-full text-left px-3 py-2 rounded-lg text-xs transition-colors text-gray-100 hover:bg-white/14 flex items-center gap-2 {track.index === selectedSubtitleIndex ? 'bg-cyan-400 bg-opacity-20 text-cyan-200' : ''}">{track.display_title}</button>
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
      <button aria-label="Toggle favorite" onclick={() => onToggleFavorite(item.id, item.is_favorite)} class="p-2.5 rounded-xl bg-white/5 hover:bg-white/12 border border-white/10 transition-all {item.is_favorite ? 'text-rose-400 hover:text-rose-300 bg-rose-500/10 border-rose-500/20' : 'text-gray-400 hover:text-white'}"><svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"/></svg></button>
      <button aria-label="Toggle watched" onclick={() => onTogglePlayed(item.id, item.played)} class="p-2.5 rounded-xl bg-white/5 hover:bg-white/12 border border-white/10 transition-all {item.played ? 'text-emerald-400 hover:text-emerald-300 bg-emerald-500/10 border-emerald-500/20' : 'text-gray-400 hover:text-white'}"><svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd"/></svg></button>
      <div class="relative">
        <button aria-label="More options" aria-haspopup="menu" aria-expanded={contextMenuOpen} onclick={() => contextMenuOpen = !contextMenuOpen} class="p-2.5 rounded-xl bg-white/5 hover:bg-white/12 border border-white/10 text-gray-400 hover:text-white transition-colors"><svg class="w-5 h-5" viewBox="0 0 20 20" fill="currentColor"><path d="M6 10a2 2 0 11-4 0 2 2 0 014 0zM12 10a2 2 0 11-4 0 2 2 0 014 0zM16 12a2 2 0 100-4 2 2 0 000 4z"/></svg></button>
        {#if contextMenuOpen}
          <button type="button" aria-label="Close options menu" onclick={closeContextMenu} class="fixed inset-0 z-40"></button>
          <div role="menu" aria-label="Item actions" class="absolute right-0 top-full mt-1.5 w-52 bg-[rgba(7,14,24,0.92)] border border-white/22 rounded-xl shadow-xl z-50 py-1.5 backdrop-blur-2xl overflow-hidden">
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
      {#if episodesLoading}
        <div class="mb-8">
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-base font-semibold text-white">Episodes</h2>
          </div>
          <div class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-5 px-5" aria-hidden="true">
            {#each Array.from({ length: 4 }) as _}
              <div class="flex-shrink-0 w-48 rounded-xl overflow-hidden">
                <div class="skeleton-card aspect-video animate-pulse bg-white/5"></div>
                <div class="p-2.5">
                  <div class="h-3 bg-white/10 rounded w-3/4 animate-pulse"></div>
                  <div class="mt-2 h-3 bg-white/5 rounded w-11/12 animate-pulse"></div>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {:else if episodes.length > 0}
        <HorizontalCarousel
          title="Episodes"
          items={episodes}
          getKey={(episode) => episode.id}
        >
          {#snippet headerExtra()}
            <div class="relative">
              <select
                onchange={(e) => { const target = e.target as HTMLSelectElement; onLoadSeasonEpisodes(target.value); }}
                value={selectedSeasonId ?? ""}
                aria-label="Select season"
                class="appearance-none bg-white/5 text-[var(--text-primary)] text-sm px-3.5 py-2 pr-9 rounded-xl border border-white/10 focus:outline-none focus:ring-2 focus:ring-cyan-400 cursor-pointer transition-all hover:bg-white/10"
              >
                {#each seasons as season (season.id)}
                  <option value={season.id} class="bg-gray-800">{season.name}</option>
                {/each}
              </select>
              <svg class="absolute right-2 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 pointer-events-none" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd"/></svg>
            </div>
          {/snippet}

          {#snippet renderCard(episode)}
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
          {/snippet}
        </HorizontalCarousel>
      {:else}
        <p class="text-gray-500 text-sm text-center py-6">No episodes found for this season.</p>
      {/if}
    {/if}

    <!-- Seasons carousel -->
    {#if seasons.length > 1}
      <HorizontalCarousel
        title="Seasons"
        items={seasons}
        getKey={(season) => season.id}
      >
        {#snippet renderCard(season)}
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
        {/snippet}
      </HorizontalCarousel>
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

    <!-- Related -->
    {#if similarItems.length > 0}
      <HorizontalCarousel
        title="Related"
        items={similarItems}
        getKey={(related) => related.id}
      >
        {#snippet renderCard(related)}
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
        {/snippet}
      </HorizontalCarousel>
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
