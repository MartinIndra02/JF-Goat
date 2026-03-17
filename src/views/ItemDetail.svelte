<script lang="ts">
  import { push, querystring } from "svelte-spa-router";
  import {
    getItemById,
    getSeriesSeasons,
    getSeasonEpisodes,
    getItemPeople,
    getSimilarItems,
    getMediaStreams,
    getExternalUrls,
    togglePlayed,
    toggleFavorite,
  } from "../lib/api";
  import type { MediaItem, Person, MediaStreamInfo, ExternalUrl } from "../lib/types";
  import EpisodeDetail from "./detail/EpisodeDetail.svelte";
  import SeriesDetail from "./detail/SeriesDetail.svelte";
  import SeasonDetail from "./detail/SeasonDetail.svelte";

  // Reactively derive the item ID from the query string
  const itemId = $derived(new URLSearchParams($querystring).get("id") ?? "");

  let item = $state<MediaItem | null>(null);
  let seasons = $state<MediaItem[]>([]);
  let episodes = $state<MediaItem[]>([]);
  let selectedSeasonId = $state<string | null>(null);
  let allSeasonEpisodes = $state<Record<string, MediaItem[]>>({});
  let loading = $state(true);
  let loadError = $state(false);
  let episodesLoading = $state(false);

  // For episode detail: sibling episodes from same season
  let siblingEpisodes = $state<MediaItem[]>([]);

  // Cast & crew
  let people = $state<Person[]>([]);

  // Similar/Related items
  let similarItems = $state<MediaItem[]>([]);

  // Media streams & external URLs
  let mediaStreams = $state<MediaStreamInfo | null>(null);
  let externalUrls = $state<ExternalUrl[]>([]);

  async function loadSeasonEpisodes(seasonId: string) {
    selectedSeasonId = seasonId;
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

  async function loadPeople(id: string) {
    try { people = await getItemPeople(id); }
    catch { people = []; }
  }

  async function loadSimilarItems(id: string) {
    try { similarItems = await getSimilarItems(id, 12); }
    catch { similarItems = []; }
  }

  async function loadMediaStreams(id: string) {
    try { mediaStreams = await getMediaStreams(id); }
    catch { mediaStreams = null; }
  }

  async function loadExternalUrls(id: string) {
    try { externalUrls = await getExternalUrls(id); }
    catch { externalUrls = []; }
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

    // Reset state for new item
    loading = true;
    loadError = false;
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

    loadItem(id);
  });

  async function loadItem(id: string) {
    try {
      const result = await getItemById(id);
      if (!result) { loadError = true; return; }
      item = result;

      // Series: load seasons and first season episodes
      if (item.type === "Series") {
        seasons = await getSeriesSeasons(item.id);
        if (seasons.length > 0) await loadSeasonEpisodes(seasons[0].id);
        loadPeople(item.id);
        loadSimilarItems(item.id);
        loadExternalUrls(item.id);
        // Load streams from resume episode if available
        const resumeEp = findResumeEpisode();
        if (resumeEp) loadMediaStreams(resumeEp.id);
      }

      // Season: load episodes for this season
      if (item.type === "Season") {
        episodes = await getSeasonEpisodes(item.id);
        if (item.series_id) seasons = await getSeriesSeasons(item.series_id);
        loadPeople(item.series_id ?? item.id);
      }

      // Episode/Movie
      if (item.type === "Episode" || item.type === "Movie") {
        if (item.type === "Episode" && item.season_id) {
          siblingEpisodes = await getSeasonEpisodes(item.season_id);
        }
        loadPeople(item.id);
        loadMediaStreams(item.id);
        loadExternalUrls(item.id);
      }
    } catch (e) {
      console.error("Failed to load item details:", e);
      loadError = true;
    } finally {
      loading = false;
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
  <main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
    <div class="text-center">
      <svg class="w-8 h-8 text-blue-400 animate-spin mx-auto mb-3" viewBox="0 0 24 24" fill="none">
        <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25"/>
        <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
      </svg>
      <p class="text-gray-400 text-sm">Loading...</p>
    </div>
  </main>

{:else if loadError}
  <main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
    <div class="text-center">
      <p class="text-gray-400 text-sm mb-4">Could not load this item. It may not be synced yet or the server is unreachable.</p>
      <div class="flex gap-3 justify-center">
        <button onclick={() => { loadError = false; loading = true; loadItem(itemId); }} class="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm font-medium transition-colors">Retry</button>
        <button onclick={() => window.history.length > 1 ? window.history.back() : push("/home")} class="px-4 py-2 bg-white/10 hover:bg-white/15 rounded-lg text-sm font-medium transition-colors">Go back</button>
      </div>
    </div>
  </main>

{:else if item && (item.type === "Episode" || item.type === "Movie")}
  <EpisodeDetail
    {item}
    {siblingEpisodes}
    {people}
    {mediaStreams}
    {externalUrls}
    onTogglePlayed={handleTogglePlayed}
    onToggleFavorite={handleToggleFavorite}
  />

{:else if item && item.type === "Series"}
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

{:else if item && item.type === "Season"}
  <SeasonDetail
    {item}
    {episodes}
    {seasons}
    {people}
    onTogglePlayed={handleTogglePlayed}
    onToggleFavorite={handleToggleFavorite}
  />

{:else if item}
  <main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
    <div class="text-center">
      <p class="text-gray-400 text-sm mb-4">Unsupported item type: {item.type}</p>
      <button onclick={() => window.history.length > 1 ? window.history.back() : push("/home")} class="text-blue-400 hover:text-blue-300 text-sm">Go back</button>
    </div>
  </main>
{/if}
