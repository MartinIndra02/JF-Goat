<script lang="ts">
  import { tick } from "svelte";
  import { location, push, querystring } from "svelte-spa-router";
  import {
    getItemById,
    getSeriesSeasons,
    getSeasonEpisodes,
    mpvPlay,
  } from "../lib/api";
  import {
    showPlayer,
    getPreferredAudioStreamIndex,
    getPreferredSubtitleStreamIndex,
    setPreferredAudioStreamIndex,
    setPreferredSubtitleStreamIndex,
    setPreferredAudioMetadata,
    setPreferredSubtitleMetadata,
  } from "../lib/stores/player.svelte";
  import { seasonNumber } from "./detail/detailHelpers";
  import {
    getItemPeople,
    getSimilarItems,
    getMediaStreams,
    getExternalUrls,
    togglePlayed,
    toggleFavorite,
  } from "../lib/api";
  import type {
    MediaItem,
    Person,
    MediaStreamInfo,
    ExternalUrl,
    PlaybackSelection,
  } from "../lib/types";
  import EpisodeDetail from "./detail/EpisodeDetail.svelte";
  import SeriesDetail from "./detail/SeriesDetail.svelte";
  import SeasonDetail from "./detail/SeasonDetail.svelte";

  const itemIdFromPath = $derived.by(() => {
    const path = $location || "";
    const match = path.match(/^\/item\/([^/?]+)/);
    return match ? decodeURIComponent(match[1]) : "";
  });

  // Resolve item ID from either /item?id=... or /item/:id
  const itemId = $derived(
    new URLSearchParams($querystring).get("id") ?? itemIdFromPath,
  );

  let item = $state<MediaItem | null>(null);
  let seasons = $state<MediaItem[]>([]);
  let episodes = $state<MediaItem[]>([]);
  let selectedSeasonId = $state<string | null>(null);
  let allSeasonEpisodes = $state<Record<string, MediaItem[]>>({});
  let loading = $state(true);
  let loadError = $state(false);
  let episodesLoading = $state(false);
  let detailContentAnchor = $state<HTMLDivElement | null>(null);
  let focusedDetailItemId = $state("");
  let activeDetailController: AbortController | null = null;
  let activeSeasonController: AbortController | null = null;

  // For episode detail: sibling episodes from same season
  let siblingEpisodes = $state<MediaItem[]>([]);

  // Cast & crew
  let people = $state<Person[]>([]);

  // Similar/Related items
  let similarItems = $state<MediaItem[]>([]);

  function isAbortError(error: unknown): boolean {
    return error instanceof DOMException && error.name === "AbortError";
  }

  async function handlePlay(
    target: MediaItem,
    fromStart: boolean = false,
    selection?: PlaybackSelection,
  ) {
    const startTicks = fromStart ? 0 : target.playback_ticks;

    let displayTitle = target.name;
    if (target.type === "Episode" && target.series_name) {
      const sNum = seasonNumber(target.season_name);
      displayTitle = `${target.series_name} - S${sNum} E${target.index_number ?? "?"} - ${target.name}`;
    }

    showPlayer(target.id, displayTitle);

    if (selection?.audioStreamIndex !== undefined) {
      if (selection.audioStreamIndex === null) {
        setPreferredAudioStreamIndex(undefined);
      } else {
        setPreferredAudioStreamIndex(selection.audioStreamIndex);
      }
      setPreferredAudioMetadata(
        selection.audioLanguage,
        selection.audioDisplayTitle,
      );
    }

    if (selection?.subtitleStreamIndex !== undefined) {
      setPreferredSubtitleStreamIndex(selection.subtitleStreamIndex);
      setPreferredSubtitleMetadata(
        selection.subtitleLanguage,
        selection.subtitleDisplayTitle,
      );
    }

    const preferredAudio = getPreferredAudioStreamIndex();
    const preferredSubtitle = getPreferredSubtitleStreamIndex();
    const resolvedAudio = selection?.audioStreamIndex ?? preferredAudio ?? null;
    const resolvedSubtitle = selection?.subtitleStreamIndex ?? preferredSubtitle;

    try {
      await mpvPlay({
        itemId: target.id,
        startTicks,
        audioStreamIndex: resolvedAudio ?? null,
        subtitleStreamIndex:
          resolvedSubtitle === undefined
            ? null
            : resolvedSubtitle === null
              ? -1
              : resolvedSubtitle,
        // Startup should always be direct-play; transcode options are applied later from player UI.
        maxStreamingBitrate: null,
        targetHeight: null,
      });
    } catch (e) {
      console.error("Failed to start playback:", e);
    }
  }

  // Media streams & external URLs
  let mediaStreams = $state<MediaStreamInfo | null>(null);
  let externalUrls = $state<ExternalUrl[]>([]);

  async function loadSeasonEpisodes(
    seasonId: string,
    signal?: AbortSignal,
  ) {
    selectedSeasonId = seasonId;
    if (allSeasonEpisodes[seasonId]) {
      episodes = allSeasonEpisodes[seasonId];
      return;
    }

    let requestSignal = signal;
    if (!requestSignal) {
      activeSeasonController?.abort();
      const seasonController = new AbortController();
      activeSeasonController = seasonController;
      requestSignal = seasonController.signal;
    }

    episodesLoading = true;
    try {
      const eps = await getSeasonEpisodes(seasonId, { signal: requestSignal });
      if (requestSignal.aborted) return;
      allSeasonEpisodes[seasonId] = eps;
      episodes = eps;
    } catch (e) {
      if (requestSignal.aborted || isAbortError(e)) return;
      console.error("Failed to load episodes:", e);
      episodes = [];
    } finally {
      if (!requestSignal.aborted) {
        episodesLoading = false;
      }
      if (activeSeasonController?.signal === requestSignal) {
        activeSeasonController = null;
      }
    }
  }

  async function loadPeople(id: string, signal: AbortSignal) {
    try {
      const result = await getItemPeople(id, { signal });
      if (signal.aborted) return;
      people = result;
    }
    catch (e) {
      if (signal.aborted || isAbortError(e)) return;
      people = [];
    }
  }

  async function loadSimilarItems(id: string, signal: AbortSignal) {
    try {
      const result = await getSimilarItems(id, 12, { signal });
      if (signal.aborted) return;
      similarItems = result;
    }
    catch (e) {
      if (signal.aborted || isAbortError(e)) return;
      similarItems = [];
    }
  }

  async function loadMediaStreams(id: string, signal: AbortSignal) {
    try {
      const result = await getMediaStreams(id, { signal });
      if (signal.aborted) return;
      mediaStreams = result;
    }
    catch (e) {
      if (signal.aborted || isAbortError(e)) return;
      mediaStreams = null;
    }
  }

  async function loadExternalUrls(id: string, signal: AbortSignal) {
    try {
      const result = await getExternalUrls(id, { signal });
      if (signal.aborted) return;
      externalUrls = result;
    }
    catch (e) {
      if (signal.aborted || isAbortError(e)) return;
      externalUrls = [];
    }
  }

  async function handleTogglePlayed(id: string, currentPlayed: boolean) {
    try {
      const newPlayed = await togglePlayed(id, currentPlayed);
      // Update main item if it matches
      if (item && item.id === id) {
        item = { ...item, played: newPlayed, playback_ticks: newPlayed ? item.playback_ticks : 0 };
      }
      // Update episodes list
      episodes = episodes.map(ep => ep.id === id ? { ...ep, played: newPlayed, playback_ticks: newPlayed ? ep.playback_ticks : 0 } : ep);
      // Update sibling episodes
      siblingEpisodes = siblingEpisodes.map(ep => ep.id === id ? { ...ep, played: newPlayed, playback_ticks: newPlayed ? ep.playback_ticks : 0 } : ep);
      // Update allSeasonEpisodes cache
      for (const seasonId of Object.keys(allSeasonEpisodes)) {
        allSeasonEpisodes[seasonId] = allSeasonEpisodes[seasonId].map(ep =>
          ep.id === id ? { ...ep, played: newPlayed, playback_ticks: newPlayed ? ep.playback_ticks : 0 } : ep
        );
      }
    } catch (e) {
      console.error("Failed to toggle played:", e);
    }
  }

  async function handleToggleFavorite(id: string, currentFavorite: boolean) {
    try {
      const newFavorite = await toggleFavorite(id, currentFavorite);
      if (item && item.id === id) {
        item = { ...item, is_favorite: newFavorite };
      }
      episodes = episodes.map(ep => ep.id === id ? { ...ep, is_favorite: newFavorite } : ep);
      siblingEpisodes = siblingEpisodes.map(ep => ep.id === id ? { ...ep, is_favorite: newFavorite } : ep);
    } catch (e) {
      console.error("Failed to toggle favorite:", e);
    }
  }

  // Reactively load item data whenever itemId changes
  $effect(() => {
    const id = itemId;
    if (!id) { push("/home"); return; }

    const detailController = new AbortController();
    activeDetailController?.abort();
    activeSeasonController?.abort();
    activeDetailController = detailController;
    activeSeasonController = null;

    // Reset state for new item
    loading = true;
    loadError = false;
    episodesLoading = false;
    item = null;
    seasons = [];
    episodes = [];
    selectedSeasonId = null;
    allSeasonEpisodes = {};
    siblingEpisodes = [];
    people = [];
    similarItems = [];
    mediaStreams = null;
    externalUrls = [];

    loadItem(id, detailController.signal);

    return () => {
      detailController.abort();
      if (activeDetailController === detailController) {
        activeDetailController = null;
      }
    };
  });

  $effect(() => {
    if (loading || !item) return;
    if (focusedDetailItemId === item.id) return;

    focusedDetailItemId = item.id;
    void tick().then(() => {
      detailContentAnchor?.focus();
    });
  });

  function retryLoadItem() {
    const id = itemId;
    if (!id) return;

    const detailController = new AbortController();
    activeDetailController?.abort();
    activeSeasonController?.abort();
    activeDetailController = detailController;
    activeSeasonController = null;

    loading = true;
    loadError = false;
    episodesLoading = false;
    item = null;
    seasons = [];
    episodes = [];
    selectedSeasonId = null;
    allSeasonEpisodes = {};
    siblingEpisodes = [];
    people = [];
    similarItems = [];
    mediaStreams = null;
    externalUrls = [];

    loadItem(id, detailController.signal);
  }

  async function loadItem(id: string, signal: AbortSignal) {
    try {
      const result = await getItemById(id, { signal });
      if (signal.aborted) return;
      if (!result) { loadError = true; return; }
      item = result;

      // Series: load seasons and first season episodes
      if (item.type === "Series") {
        seasons = await getSeriesSeasons(item.id, { signal });
        if (signal.aborted) return;
        if (seasons.length > 0) {
          await loadSeasonEpisodes(seasons[0].id, signal);
          if (signal.aborted) return;
        }
        loadPeople(item.id, signal);
        loadSimilarItems(item.id, signal);
        loadExternalUrls(item.id, signal);
        // Load streams from resume episode if available
        const resumeEp = findResumeEpisode();
        if (resumeEp) {
          loadMediaStreams(resumeEp.id, signal);
        }
      }

      // Season: load episodes for this season
      if (item.type === "Season") {
        episodes = await getSeasonEpisodes(item.id, { signal });
        if (signal.aborted) return;
        if (item.series_id) {
          seasons = await getSeriesSeasons(item.series_id, { signal });
          if (signal.aborted) return;
        }
        loadPeople(item.series_id ?? item.id, signal);
      }

      // Episode/Movie
      if (item.type === "Episode" || item.type === "Movie") {
        if (item.type === "Episode" && item.season_id) {
          siblingEpisodes = await getSeasonEpisodes(item.season_id, { signal });
          if (signal.aborted) return;
        }
        loadPeople(item.id, signal);
        loadMediaStreams(item.id, signal);
        loadExternalUrls(item.id, signal);
      }
    } catch (e) {
      if (signal.aborted || isAbortError(e)) return;
      console.error("Failed to load item details:", e);
      loadError = true;
    } finally {
      if (!signal.aborted) {
        loading = false;
      }
    }
  }

  function findResumeEpisode(): MediaItem | null {
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
  }
</script>

{#if loading}
  <main class="min-h-screen bg-gray-900 text-white" aria-hidden="true">
    <section class="relative h-[42vh] min-h-[260px] overflow-hidden">
      <div class="absolute inset-0 skeleton-card rounded-none border-none"></div>
      <div class="absolute inset-0 bg-gradient-to-t from-gray-900 via-gray-900/45 to-transparent"></div>

      <div class="absolute bottom-0 left-0 right-0 px-6 pb-8 space-y-3">
        <div class="skeleton-pill w-24"></div>
        <div class="skeleton-line skeleton-line--title w-72 max-w-[85%]"></div>
        <div class="skeleton-line skeleton-line--text w-full max-w-2xl"></div>
        <div class="skeleton-line skeleton-line--text skeleton-line--muted w-3/4 max-w-xl"></div>
        <div class="flex flex-wrap gap-2 pt-1">
          {#each Array.from({ length: 4 }) as _}
            <div class="skeleton-pill w-20"></div>
          {/each}
        </div>
      </div>
    </section>

    <section class="px-6 py-6 space-y-6">
      <div class="space-y-3">
        <div class="skeleton-line skeleton-line--title w-48"></div>
        <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-3">
          {#each Array.from({ length: 10 }) as _, cardIndex}
            <div aria-label={`Loading list card ${cardIndex + 1}`}>
              <div class="skeleton-card aspect-video"></div>
              <div class="mt-2 skeleton-line skeleton-line--text w-11/12"></div>
              <div class="mt-1 skeleton-line skeleton-line--text skeleton-line--muted w-2/3"></div>
            </div>
          {/each}
        </div>
      </div>

      <div class="space-y-3">
        <div class="skeleton-line skeleton-line--title w-36"></div>
        <div class="grid grid-cols-3 sm:grid-cols-5 md:grid-cols-7 lg:grid-cols-9 gap-3">
          {#each Array.from({ length: 9 }) as _, personIndex}
            <div class="space-y-2" aria-label={`Loading person ${personIndex + 1}`}>
              <div class="skeleton-card aspect-square rounded-full"></div>
              <div class="mx-auto skeleton-line skeleton-line--text w-5/6"></div>
            </div>
          {/each}
        </div>
      </div>
    </section>
  </main>

{:else if loadError}
  <main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
    <div class="text-center">
      <p class="text-gray-400 text-sm mb-4">Could not load this item. It may not be synced yet or the server is unreachable.</p>
      <div class="flex gap-3 justify-center">
        <button onclick={retryLoadItem} class="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm font-medium transition-colors">Retry</button>
        <button onclick={() => window.history.length > 1 ? window.history.back() : push("/home")} class="px-4 py-2 bg-white/10 hover:bg-white/15 rounded-lg text-sm font-medium transition-colors">Go back</button>
      </div>
    </div>
  </main>

{:else if item && (item.type === "Episode" || item.type === "Movie")}
  <div bind:this={detailContentAnchor} tabindex="-1" class="focus:outline-none">
    <EpisodeDetail
      {item}
      {siblingEpisodes}
      {people}
      {mediaStreams}
      {externalUrls}
      onPlay={handlePlay}
      onTogglePlayed={handleTogglePlayed}
      onToggleFavorite={handleToggleFavorite}
    />
  </div>

{:else if item && item.type === "Series"}
  <div bind:this={detailContentAnchor} tabindex="-1" class="focus:outline-none">
    <SeriesDetail
      {item}
      {seasons}
      {episodes}
      {allSeasonEpisodes}
      {selectedSeasonId}
      {people}
      {similarItems}
      {mediaStreams}
      {externalUrls}
      {episodesLoading}
      onLoadSeasonEpisodes={loadSeasonEpisodes}
      onTogglePlayed={handleTogglePlayed}
      onToggleFavorite={handleToggleFavorite}
    />
  </div>

{:else if item && item.type === "Season"}
  <div bind:this={detailContentAnchor} tabindex="-1" class="focus:outline-none">
    <SeasonDetail
      {item}
      {episodes}
      {people}
      onTogglePlayed={handleTogglePlayed}
      onToggleFavorite={handleToggleFavorite}
    />
  </div>

{:else if item}
  <main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
    <div class="text-center">
      <p class="text-gray-400 text-sm mb-4">Unsupported item type: {item.type}</p>
      <button onclick={() => window.history.length > 1 ? window.history.back() : push("/home")} class="text-blue-400 hover:text-blue-300 text-sm">Go back</button>
    </div>
  </main>
{/if}
