<script lang="ts">
  import { onMount } from "svelte";
  import {
    logout as logoutApi,
    startSync,
    searchItems,
    getRecentMovies,
    getRecentSeries,
    getContinueWatching,
    getLatestMedia,
  } from "../lib/api";
  import { getSession, setUnauthenticated } from "../lib/stores/auth.svelte";
  import { initSyncListeners, getSyncState, resetSyncStore } from "../lib/stores/sync.svelte";
  import Button from "../components/ui/Button.svelte";
  import TextInput from "../components/ui/TextInput.svelte";
  import SyncIndicator from "../components/layout/SyncIndicator.svelte";
  import HeroBanner from "../components/media/HeroBanner.svelte";
  import MediaRow from "../components/media/MediaRow.svelte";
  import PosterCard from "../components/media/PosterCard.svelte";
  import { push } from "svelte-spa-router";
  import type { MediaItem } from "../lib/types";

  const session = getSession();

  let searchQuery = $state("");
  let searchResults = $state<MediaItem[]>([]);
  let searchSource = $state<string>("");
  let searching = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  let featuredItems = $state<MediaItem[]>([]);
  let continueWatching = $state<MediaItem[]>([]);
  let recentMovies = $state<MediaItem[]>([]);
  let recentSeries = $state<MediaItem[]>([]);
  let loading = $state(true);

  const syncState = $derived(getSyncState());

  initSyncListeners();
  startSync().catch((e) => console.error("Failed to start sync:", e));

  onMount(() => {
    loadDashboard();
  });

  async function loadDashboard() {
    try {
      const [featured, watching, movies, series] = await Promise.all([
        getLatestMedia(10).catch(() => []),
        getContinueWatching(20).catch(() => []),
        getRecentMovies(20).catch(() => []),
        getRecentSeries(20).catch(() => []),
      ]);
      featuredItems = featured;
      continueWatching = watching;
      recentMovies = movies;
      recentSeries = series;
    } catch (e) {
      console.error("Failed to load dashboard:", e);
    } finally {
      loading = false;
    }
  }

  async function handleSearch() {
    const query = searchQuery.trim();
    if (!query) {
      searchResults = [];
      searchSource = "";
      return;
    }

    searching = true;
    try {
      const result = await searchItems(query);
      searchResults = result.items;
      searchSource = result.source;
    } catch (e) {
      console.error("Search failed:", e);
      searchResults = [];
    } finally {
      searching = false;
    }
  }

  function onSearchInput() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(handleSearch, 300);
  }

  async function handleLogout() {
    try {
      await logoutApi();
    } catch {
      // Best effort
    }
    resetSyncStore();
    setUnauthenticated();
    push("/connect");
  }
</script>

<main class="min-h-screen bg-gray-900 text-white pb-16">
  <!-- Top bar -->
  <header class="sticky top-0 z-40 bg-gray-900/80 backdrop-blur-md border-b border-white/5">
    <div class="flex items-center gap-4 px-6 py-3">
      <div class="flex items-center gap-3 min-w-0">
        <div class="w-8 h-8 rounded-lg bg-blue-600 flex items-center justify-center shrink-0">
          <svg class="w-4 h-4 text-white" viewBox="0 0 24 24" fill="currentColor">
            <path d="M4 6h16v2H4zm0 5h16v2H4zm0 5h16v2H4z"/>
          </svg>
        </div>
        <div class="min-w-0">
          <h1 class="text-sm font-semibold truncate">
            {#if session?.server_name}
              {session.server_name}
            {:else}
              jfFast
            {/if}
          </h1>
          <p class="text-xs text-gray-500 truncate">
            {session?.username ?? ""}
          </p>
        </div>
      </div>

      <div class="flex-1 max-w-md mx-auto">
        <div class="relative">
          <svg class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500" viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z" clip-rule="evenodd"/>
          </svg>
          <TextInput
            bind:value={searchQuery}
            placeholder="Search your library..."
            oninput={onSearchInput}
          />
        </div>
      </div>

      <Button variant="secondary" onclick={handleLogout}>
        <span class="text-sm">Log Out</span>
      </Button>
    </div>
  </header>

  {#if searchQuery.trim()}
    <!-- Search results -->
    <div class="px-6 pt-6">
      {#if searching}
        <p class="text-gray-400 text-sm">Searching...</p>
      {:else if searchResults.length === 0}
        <p class="text-gray-400 text-sm">No results found.</p>
      {:else}
        <p class="text-gray-500 text-xs mb-4">
          {searchResults.length} results (from {searchSource === "remote" ? "server" : "local cache"})
        </p>
        <div class="flex flex-wrap gap-3">
          {#each searchResults as item (item.id)}
            <PosterCard {item} />
          {/each}
        </div>
      {/if}
    </div>
  {:else if loading}
    <div class="flex items-center justify-center h-64">
      <div class="text-center">
        <svg class="w-8 h-8 text-blue-400 animate-spin mx-auto mb-3" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25"/>
          <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
        </svg>
        <p class="text-gray-400 text-sm">Loading your library...</p>
      </div>
    </div>
  {:else}
    <!-- Dashboard content -->
    <div class="space-y-2">
      <!-- Hero Banner -->
      <HeroBanner items={featuredItems} />

      <!-- Continue Watching -->
      <MediaRow title="Continue Watching" items={continueWatching} landscape={true} />

      <!-- Recently Added Movies -->
      <MediaRow title="Recently Added Movies" items={recentMovies} />

      <!-- Recently Added Series -->
      <MediaRow title="Recently Added Series" items={recentSeries} />

      {#if !featuredItems.length && !continueWatching.length && !recentMovies.length && !recentSeries.length}
        <div class="flex flex-col items-center justify-center h-64 text-center px-6">
          <svg class="w-16 h-16 text-gray-700 mb-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M7 4v16M17 4v16M3 8h4m10 0h4M3 12h18M3 16h4m10 0h4M4 20h16a1 1 0 001-1V5a1 1 0 00-1-1H4a1 1 0 00-1 1v14a1 1 0 001 1z"/>
          </svg>
          <p class="text-gray-400 text-lg font-medium mb-1">Your library is empty</p>
          <p class="text-gray-600 text-sm">Sync may still be in progress. Content will appear here once indexed.</p>
        </div>
      {/if}
    </div>
  {/if}
</main>

<SyncIndicator />