<script lang="ts">
  import { onMount } from "svelte";
  import {
    logout as logoutApi,
    startSync,
    searchItems,
    getUserViews,
    getResumeItems,
    getNextUp,
    getLatestItems,
    getLatestMedia,
    loadHomepageCache,
    saveHomepageCache,
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
  import type { MediaItem, UserLibrary, HomepageCache } from "../lib/types";

  const session = getSession();

  let searchQuery = $state("");
  let searchResults = $state<MediaItem[]>([]);
  let searchSource = $state<string>("");
  let searching = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  // Real-time data from Jellyfin server
  let resumeItems = $state<MediaItem[]>([]);
  let nextUpItems = $state<MediaItem[]>([]);
  let userLibraries = $state<UserLibrary[]>([]);
  let libraryLatest = $state<Record<string, MediaItem[]>>({});
  let featuredItems = $state<MediaItem[]>([]);
  // Show loading spinner only if we have no cached data to show
  let loading = $state(true);
  let hasCachedData = $state(false);

  const syncState = $derived(getSyncState());

  initSyncListeners();
  startSync().catch((e) => console.error("Failed to start sync:", e));

  onMount(() => {
    loadCachedThenRefresh();
  });

  async function loadCachedThenRefresh() {
    // Phase 1: Load cached homepage data instantly (no network, ~0ms)
    try {
      const cached = await loadHomepageCache();
      if (cached) {
        resumeItems = cached.resume_items;
        nextUpItems = cached.next_up_items;
        userLibraries = cached.user_libraries;
        libraryLatest = cached.library_latest;
        featuredItems = cached.featured_items;
        hasCachedData = true;
        // Remove loading spinner immediately — user sees cached content
        loading = false;
      }
    } catch (e) {
      console.error("Failed to load homepage cache:", e);
    }

    // Phase 2: Fetch fresh data from the server in the background
    await refreshFromServer();
  }

  async function refreshFromServer() {
    try {
      // Fetch all real-time data from the Jellyfin server in parallel
      const [resume, nextUp, views, featured] = await Promise.all([
        getResumeItems(20).catch(() => []),
        getNextUp(20).catch(() => []),
        getUserViews().catch(() => []),
        getLatestMedia(10).catch(() => []),
      ]);

      // Silently update the UI with fresh data
      resumeItems = resume;
      nextUpItems = nextUp;
      userLibraries = views;
      featuredItems = featured;

      // Fetch latest items for each library in parallel
      const latestMap: Record<string, MediaItem[]> = {};
      if (views.length > 0) {
        const latestPromises = views.map(async (lib) => {
          const items = await getLatestItems(lib.id, 16).catch(() => []);
          return { id: lib.id, items };
        });

        const results = await Promise.all(latestPromises);
        for (const r of results) {
          latestMap[r.id] = r.items;
        }
        libraryLatest = latestMap;
      }

      // Persist fresh data to disk for next startup
      saveHomepageCache({
        resume_items: resume,
        next_up_items: nextUp,
        user_libraries: views,
        library_latest: latestMap,
        featured_items: featured,
      }).catch((e) => console.error("Failed to save homepage cache:", e));

      // Pre-cache detail page images for continue watching and next up items
      // so that detail pages load instantly when clicked
      prefetchDetailImages([...resume, ...nextUp]);
    } catch (e) {
      console.error("Failed to refresh dashboard:", e);
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

  function hasAnyContent(): boolean {
    if (featuredItems.length > 0) return true;
    if (resumeItems.length > 0) return true;
    if (nextUpItems.length > 0) return true;
    for (const lib of userLibraries) {
      if (libraryLatest[lib.id]?.length > 0) return true;
    }
    return false;
  }

  /**
   * Pre-cache images for items that appear in continue watching and next up.
   * This triggers the jfimage protocol handler to fetch and cache images
   * in the background, so detail pages load instantly when clicked.
   */
  function prefetchDetailImages(items: MediaItem[]) {
    const seen = new Set<string>();
    for (const item of items) {
      // Pre-cache poster images
      if (item.image_tag && !seen.has(`poster-${item.id}`)) {
        seen.add(`poster-${item.id}`);
        const img = new Image();
        img.src = `http://jfimage.localhost/poster/${item.id}?tag=${item.image_tag}`;
      }
      // Pre-cache backdrop images
      if (item.backdrop_tag && !seen.has(`backdrop-${item.id}`)) {
        seen.add(`backdrop-${item.id}`);
        const img = new Image();
        img.src = `http://jfimage.localhost/backdrop/${item.id}?tag=${item.backdrop_tag}`;
      }
      // Pre-cache series poster for episodes
      if (item.series_id && !seen.has(`poster-${item.series_id}`)) {
        seen.add(`poster-${item.series_id}`);
        const img = new Image();
        img.src = `http://jfimage.localhost/poster/${item.series_id}?tag=${item.series_id}`;
      }
    }
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
  {:else if loading && !hasCachedData}
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
    <!-- Dashboard content with real data -->
    <div class="space-y-2">
      <!-- Hero Banner -->
      <HeroBanner items={featuredItems} />

      <!-- Continue Watching (from Jellyfin /Users/{id}/Items/Resume) -->
      <MediaRow title="Continue Watching" items={resumeItems} landscape={true} />

      <!-- Next Up (from Jellyfin /Shows/NextUp) -->
      <MediaRow title="Next Up" items={nextUpItems} landscape={true} />

      <!-- Latest items per user library -->
      {#each userLibraries as library (library.id)}
        {#if libraryLatest[library.id]?.length}
          <MediaRow title="Latest in {library.name}" items={libraryLatest[library.id]} />
        {/if}
      {/each}

      {#if !hasAnyContent()}
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