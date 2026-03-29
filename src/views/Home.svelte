<script lang="ts">
  import { onMount } from "svelte";
  import { location, push, querystring, replace } from "svelte-spa-router";
  import {
    forceResync,
    getLatestItems,
    getLatestMedia,
    getLibraryItems,
    getNextUp,
    getResumeItems,
    getUserViews,
    loadHomepageCache,
    logout as logoutApi,
    saveHomepageCache,
    searchItems,
    startSync,
  } from "../lib/api";
  import {
    filterSuppressedNextUp,
    HOMEPAGE_CACHE_UPDATED_EVENT,
  } from "../lib/homepageFreshness";
  import { getSession, setUnauthenticated } from "../lib/stores/auth.svelte";
  import {
    isPreferencesLoaded,
    isPreferencesSaving,
    getPreferences,
    loadPreferences,
    updatePreferences,
  } from "../lib/stores/preferences.svelte";
  import {
    getLastNetworkError,
    isDegraded,
    isOnline,
    markDegraded,
    markHealthy,
  } from "../lib/stores/connectivity.svelte";
  import { pushErrorToast, pushToast } from "../lib/stores/toast.svelte";
  import { initSyncListeners, resetSyncStore } from "../lib/stores/sync.svelte";
  import SyncIndicator from "../components/layout/SyncIndicator.svelte";
  import HeroBanner from "../components/media/HeroBanner.svelte";
  import MediaRow from "../components/media/MediaRow.svelte";
  import PosterCard from "../components/media/PosterCard.svelte";
  import Button from "../components/ui/Button.svelte";
  import TextInput from "../components/ui/TextInput.svelte";
  import type { HomepageCache, MediaItem, UserLibrary } from "../lib/types";

  const session = getSession();

  let searchQuery = $state("");
  let searchResults = $state<MediaItem[]>([]);
  let searchSource = $state("");
  let searching = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let lastSearchTerm = "";
  let searchRequestId = 0;

  let resumeItems = $state<MediaItem[]>([]);
  let nextUpItems = $state<MediaItem[]>([]);
  let userLibraries = $state<UserLibrary[]>([]);
  let libraryLatest = $state<Record<string, MediaItem[]>>({});
  let featuredItems = $state<MediaItem[]>([]);
  let loading = $state(true);
  let hasCachedData = $state(false);

  let libraryItems = $state<MediaItem[]>([]);
  let libraryLoading = $state(false);
  let libraryError = $state("");
  let lastLoadedLibraryId = "";

  let runningResync = $state(false);
  let refreshTimer: ReturnType<typeof setInterval> | null = null;
  let staleClockTimer: ReturnType<typeof setInterval> | null = null;
  let cacheFreshnessNow = $state(Date.now());
  let lastDataRefreshAt = $state<number | null>(null);

  const currentPath = $derived($location || "/home");
  const routeQuery = $derived(new URLSearchParams($querystring));

  const isHomeRoute = $derived(currentPath === "/home");
  const isLibraryRoute = $derived(currentPath === "/library");
  const isSearchRoute = $derived(currentPath === "/search");
  const isSettingsRoute = $derived(currentPath === "/settings");

  const movieResults = $derived(searchResults.filter((item) => item.type === "Movie"));
  const showResults = $derived(
    searchResults.filter((item) => item.type === "Series" || item.type === "Season"),
  );
  const episodeResults = $derived(searchResults.filter((item) => item.type === "Episode"));
  const otherResults = $derived(
    searchResults.filter(
      (item) =>
        item.type !== "Movie"
        && item.type !== "Series"
        && item.type !== "Season"
        && item.type !== "Episode",
    ),
  );

  const selectedLibraryId = $derived(routeQuery.get("view") ?? "");
  const selectedLibraryLayout = $derived(normalizeLayout(routeQuery.get("layout")));
  const selectedLibrarySort = $derived(normalizeSort(routeQuery.get("sort")));
  const selectedLibraryTypeFilter = $derived(normalizeTypeFilter(routeQuery.get("type")));
  const selectedLibraryStatusFilter = $derived(normalizeStatusFilter(routeQuery.get("status")));

  const selectedLibrary = $derived(
    userLibraries.find((library) => library.id === selectedLibraryId) ?? null,
  );

  const filteredLibraryItems = $derived.by(() => {
    let items = [...libraryItems];

    if (selectedLibraryTypeFilter !== "all") {
      const mappedType = mapTypeFilterToMediaType(selectedLibraryTypeFilter);
      items = items.filter((item) => item.type === mappedType);
    }

    if (selectedLibraryStatusFilter === "unplayed") {
      items = items.filter((item) => !item.played);
    } else if (selectedLibraryStatusFilter === "in-progress") {
      items = items.filter((item) => item.playback_ticks > 0 && !item.played);
    } else if (selectedLibraryStatusFilter === "favorites") {
      items = items.filter((item) => item.is_favorite);
    }

    items.sort((a, b) => compareLibraryItems(a, b, selectedLibrarySort));
    return items;
  });

  const preferences = $derived(getPreferences());
  const preferencesLoaded = $derived(isPreferencesLoaded());
  const preferencesSaving = $derived(isPreferencesSaving());

  const online = $derived(isOnline());
  const degraded = $derived(isDegraded());
  const lastNetworkError = $derived(getLastNetworkError());
  const staleThresholdMs = $derived((preferences.refresh_interval_seconds + 60) * 1000);
  const staleAgeMs = $derived(lastDataRefreshAt ? cacheFreshnessNow - lastDataRefreshAt : null);
  const staleData = $derived(staleAgeMs !== null && staleAgeMs > staleThresholdMs);

  const statusMessage = $derived.by(() => {
    if (!online) {
      return "Offline: showing cached content and pausing live refresh.";
    }

    if (degraded && lastNetworkError) {
      return `Degraded connection: ${lastNetworkError}`;
    }

    if (staleData) {
      return `Data may be stale (${formatRelativeAge(staleAgeMs)} old).`;
    }

    return "";
  });

  initSyncListeners();
  startSync().catch((error) => {
    pushErrorToast("sync", error, "Sync startup failed", "sync-start-failed");
  });

  onMount(() => {
    const onHomepageCacheUpdated = (event: Event) => {
      const updated = (event as CustomEvent<HomepageCache>).detail;
      if (!updated) return;

      applyHomepageData(updated);
      hasCachedData = true;
      loading = false;
    };

    window.addEventListener(
      HOMEPAGE_CACHE_UPDATED_EVENT,
      onHomepageCacheUpdated as EventListener,
    );

    staleClockTimer = setInterval(() => {
      cacheFreshnessNow = Date.now();
    }, 30_000);

    void initializeHome();

    return () => {
      window.removeEventListener(
        HOMEPAGE_CACHE_UPDATED_EVENT,
        onHomepageCacheUpdated as EventListener,
      );

      if (searchTimer) clearTimeout(searchTimer);
      if (refreshTimer) clearInterval(refreshTimer);
      if (staleClockTimer) clearInterval(staleClockTimer);
    };
  });

  async function initializeHome() {
    if (!preferencesLoaded) {
      await loadPreferences();
    }

    await loadCachedThenRefresh();
    resetAutoRefreshTimer(preferences.refresh_interval_seconds);
  }

  function resetAutoRefreshTimer(seconds: number) {
    if (refreshTimer) {
      clearInterval(refreshTimer);
      refreshTimer = null;
    }

    const intervalMs = Math.max(30, seconds) * 1000;
    refreshTimer = setInterval(() => {
      if (!isHomeRoute) return;
      void refreshFromServer();
    }, intervalMs);
  }

  $effect(() => {
    if (!preferencesLoaded) return;
    resetAutoRefreshTimer(preferences.refresh_interval_seconds);
  });

  $effect(() => {
    if (!isSearchRoute) return;
    const routeSearch = (routeQuery.get("q") ?? "").trim();

    if (searchQuery !== routeSearch) {
      searchQuery = routeSearch;
    }

    if (!routeSearch) {
      lastSearchTerm = "";
      searching = false;
      searchResults = [];
      searchSource = "";
      return;
    }

    if (routeSearch !== lastSearchTerm) {
      void runSearch(routeSearch);
    }
  });

  $effect(() => {
    if (!isLibraryRoute) return;
    if (selectedLibraryId) return;
    if (userLibraries.length === 0) return;

    replace(
      `/library?view=${encodeURIComponent(userLibraries[0].id)}&layout=grid&sort=recent&type=all&status=all`,
    );
  });

  $effect(() => {
    if (!isLibraryRoute) return;
    const viewId = selectedLibraryId;
    if (!viewId) return;
    if (viewId === lastLoadedLibraryId && libraryItems.length > 0) return;
    void loadLibraryItems(viewId);
  });

  $effect(() => {
    if (typeof localStorage === "undefined") return;

    localStorage.setItem(
      "jfgoat.player.defaultPlaybackRate",
      String(preferences.playback.default_playback_rate),
    );
    localStorage.setItem(
      "jfgoat.player.defaultQualityKey",
      preferences.quality.default_quality_key,
    );

    if (preferences.language.preferred_audio_language) {
      localStorage.setItem(
        "jfgoat.player.preferredAudioLanguage",
        preferences.language.preferred_audio_language,
      );
    }

    if (preferences.language.preferred_subtitle_language) {
      localStorage.setItem(
        "jfgoat.player.preferredSubtitleLanguage",
        preferences.language.preferred_subtitle_language,
      );
    }
  });

  function applyHomepageData(data: HomepageCache) {
    resumeItems = data.resume_items;
    nextUpItems = filterSuppressedNextUp(data.next_up_items);
    userLibraries = data.user_libraries;
    libraryLatest = data.library_latest;
    featuredItems = data.featured_items;

    if (typeof data.cache_refreshed_at_epoch_ms === "number") {
      lastDataRefreshAt = data.cache_refreshed_at_epoch_ms;
    }
  }

  async function loadCachedThenRefresh() {
    if (preferences.cache.enabled) {
      try {
        const cached = await loadHomepageCache();
        if (cached) {
          applyHomepageData(cached);
          hasCachedData = true;
          loading = false;
        }
      } catch (error) {
        pushErrorToast("api", error, "Cache load failed", "homepage-cache-load-failed");
      }
    }

    await refreshFromServer();
  }

  async function refreshFromServer() {
    if (!online) {
      loading = false;
      return;
    }

    try {
      const [resume, nextUp, views, featured] = await Promise.all([
        getResumeItems(20),
        getNextUp(20),
        getUserViews(),
        getLatestMedia(10),
      ]);

      const filteredNextUp = filterSuppressedNextUp(nextUp);

      resumeItems = resume;
      nextUpItems = filteredNextUp;
      userLibraries = views;
      featuredItems = featured;

      const latestMap: Record<string, MediaItem[]> = {};
      if (views.length > 0) {
        const latestPromises = views.map(async (library) => {
          const items = await getLatestItems(library.id, 16).catch(() => []);
          return { id: library.id, items };
        });

        const results = await Promise.all(latestPromises);
        for (const result of results) {
          latestMap[result.id] = result.items;
        }
        libraryLatest = latestMap;
      }

      const refreshedAt = Date.now();
      lastDataRefreshAt = refreshedAt;
      markHealthy();

      if (preferences.cache.enabled) {
        await saveHomepageCache({
          resume_items: resume,
          next_up_items: filteredNextUp,
          user_libraries: views,
          library_latest: latestMap,
          featured_items: featured,
          cache_refreshed_at_epoch_ms: refreshedAt,
        });
      }

      prefetchDetailImages([...resume, ...filteredNextUp]);
    } catch (error) {
      markDegraded(String(error));
      pushErrorToast(
        "api",
        error,
        "Live refresh failed",
        "dashboard-refresh-failed",
      );
    } finally {
      loading = false;
    }
  }

  async function loadLibraryItems(viewId: string) {
    libraryLoading = true;
    libraryError = "";

    try {
      const items = await getLibraryItems(viewId, 250);
      libraryItems = items;
      lastLoadedLibraryId = viewId;
    } catch (error) {
      markDegraded(String(error));
      libraryError = "Could not load this library. Check your connection and try again.";
      libraryItems = [];
      lastLoadedLibraryId = "";
    } finally {
      libraryLoading = false;
    }
  }

  async function runSearch(query: string) {
    const trimmed = query.trim();
    if (!trimmed) return;

    const requestId = ++searchRequestId;
    searching = true;
    lastSearchTerm = trimmed;

    try {
      const result = await searchItems(trimmed);
      if (requestId !== searchRequestId) return;
      searchResults = result.items;
      searchSource = result.source;
    } catch (error) {
      if (requestId !== searchRequestId) return;
      pushErrorToast("api", error, "Search failed", "search-failed");
      searchResults = [];
      searchSource = "";
    } finally {
      if (requestId === searchRequestId) {
        searching = false;
      }
    }
  }

  function onSearchInput() {
    if (searchTimer) clearTimeout(searchTimer);

    searchTimer = setTimeout(() => {
      const trimmed = searchQuery.trim();
      if (!trimmed) {
        if (isSearchRoute) {
          replace("/search");
        }
        searchResults = [];
        searchSource = "";
        lastSearchTerm = "";
        return;
      }

      const target = `/search?q=${encodeURIComponent(trimmed)}`;
      if (isSearchRoute) {
        replace(target);
      } else {
        push(target);
      }
    }, 250);
  }

  function navigateTo(path: string) {
    if (currentPath === path) return;
    push(path);
  }

  function openLibraryView(library: UserLibrary) {
    push(
      `/library?view=${encodeURIComponent(library.id)}&layout=grid&sort=recent&type=all&status=all`,
    );
  }

  function updateLibraryQuery(changes: Record<string, string>) {
    const params = new URLSearchParams($querystring);
    for (const [key, value] of Object.entries(changes)) {
      params.set(key, value);
    }
    replace(`/library?${params.toString()}`);
  }

  async function handleResync() {
    if (runningResync) return;
    if (!online) {
      pushToast({
        level: "warning",
        source: "sync",
        title: "Offline",
        message: "Reconnect before forcing a resync.",
        dedupeKey: "resync-offline",
      });
      return;
    }

    runningResync = true;
    try {
      await forceResync();
      await refreshFromServer();
      if (isLibraryRoute && selectedLibraryId) {
        await loadLibraryItems(selectedLibraryId);
      }
      pushToast({
        level: "success",
        source: "sync",
        title: "Resync started",
        message: "Your library is syncing in the background.",
        dedupeKey: "resync-started",
      });
    } catch (error) {
      pushErrorToast("sync", error, "Resync failed", "force-resync-failed");
    } finally {
      runningResync = false;
    }
  }

  async function handleLogout() {
    try {
      await logoutApi();
    } catch {
      // Best effort only.
    }

    resetSyncStore();
    setUnauthenticated();
    push("/connect");
  }

  function hasAnyContent(): boolean {
    if (featuredItems.length > 0) return true;
    if (resumeItems.length > 0) return true;
    if (nextUpItems.length > 0) return true;

    for (const library of userLibraries) {
      if (libraryLatest[library.id]?.length > 0) {
        return true;
      }
    }
    return false;
  }

  function formatRuntime(ticks: number | null): string {
    if (!ticks) return "";
    const minutes = Math.round(ticks / 600_000_000);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  }

  function formatRelativeAge(ageMs: number | null): string {
    if (!ageMs || ageMs <= 0) return "just now";

    const minutes = Math.floor(ageMs / 60_000);
    if (minutes < 1) return "under a minute";
    if (minutes < 60) return `${minutes}m`;

    const hours = Math.floor(minutes / 60);
    const remMinutes = minutes % 60;
    if (hours < 24) {
      return remMinutes > 0 ? `${hours}h ${remMinutes}m` : `${hours}h`;
    }

    const days = Math.floor(hours / 24);
    return `${days}d`;
  }

  function mapTypeFilterToMediaType(typeFilter: string): string {
    if (typeFilter === "movie") return "Movie";
    if (typeFilter === "series") return "Series";
    if (typeFilter === "season") return "Season";
    if (typeFilter === "episode") return "Episode";
    return "";
  }

  function compareLibraryItems(a: MediaItem, b: MediaItem, sortBy: string): number {
    if (sortBy === "title-asc") return a.name.localeCompare(b.name);
    if (sortBy === "title-desc") return b.name.localeCompare(a.name);
    if (sortBy === "rating-desc") return (b.community_rating ?? 0) - (a.community_rating ?? 0);
    if (sortBy === "year-desc") return (b.production_year ?? 0) - (a.production_year ?? 0);

    const dateA = parseIsoDate(a.date_updated) ?? parseIsoDate(a.date_created) ?? 0;
    const dateB = parseIsoDate(b.date_updated) ?? parseIsoDate(b.date_created) ?? 0;
    return dateB - dateA;
  }

  function parseIsoDate(value: string | null): number | null {
    if (!value) return null;
    const parsed = Date.parse(value);
    return Number.isNaN(parsed) ? null : parsed;
  }

  function normalizeLayout(value: string | null): "grid" | "list" {
    return value === "list" ? "list" : "grid";
  }

  function normalizeSort(
    value: string | null,
  ): "recent" | "title-asc" | "title-desc" | "rating-desc" | "year-desc" {
    if (
      value === "recent"
      || value === "title-asc"
      || value === "title-desc"
      || value === "rating-desc"
      || value === "year-desc"
    ) {
      return value;
    }
    return "recent";
  }

  function normalizeTypeFilter(
    value: string | null,
  ): "all" | "movie" | "series" | "season" | "episode" {
    if (
      value === "all"
      || value === "movie"
      || value === "series"
      || value === "season"
      || value === "episode"
    ) {
      return value;
    }
    return "all";
  }

  function normalizeStatusFilter(
    value: string | null,
  ): "all" | "unplayed" | "in-progress" | "favorites" {
    if (
      value === "all"
      || value === "unplayed"
      || value === "in-progress"
      || value === "favorites"
    ) {
      return value;
    }
    return "all";
  }

  function prefetchDetailImages(items: MediaItem[]) {
    const seen = new Set<string>();

    for (const item of items) {
      if (item.image_tag && !seen.has(`poster-${item.id}`)) {
        seen.add(`poster-${item.id}`);
        const img = new Image();
        img.src = `http://jfimage.localhost/poster/${item.id}?tag=${item.image_tag}`;
      }

      if (item.backdrop_tag && !seen.has(`backdrop-${item.id}`)) {
        seen.add(`backdrop-${item.id}`);
        const img = new Image();
        img.src = `http://jfimage.localhost/backdrop/${item.id}?tag=${item.backdrop_tag}`;
      }

      if (item.series_id && !seen.has(`poster-${item.series_id}`)) {
        seen.add(`poster-${item.series_id}`);
        const img = new Image();
        img.src = `http://jfimage.localhost/poster/${item.series_id}?tag=${item.series_id}`;
      }
    }
  }

  function setAutoplayPreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      playback: {
        autoplay_next_episode: target.checked,
      },
    });
  }

  function setPlaybackRatePreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      playback: {
        default_playback_rate: Number(target.value),
      },
    });
  }

  function setAudioLanguagePreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      language: {
        preferred_audio_language: target.value,
      },
    });
  }

  function setSubtitleLanguagePreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      language: {
        preferred_subtitle_language: target.value,
      },
    });
  }

  function setQualityPreference(event: Event) {
    const target = event.target as HTMLSelectElement;
    updatePreferences({
      quality: {
        default_quality_key: target.value,
      },
    });
  }

  function setCacheEnabledPreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      cache: {
        enabled: target.checked,
      },
    });
  }

  function setCacheAgePreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      cache: {
        max_age_minutes: Number(target.value),
      },
    });
  }

  function setRefreshIntervalPreference(event: Event) {
    const target = event.target as HTMLSelectElement;
    updatePreferences({
      refresh_interval_seconds: Number(target.value),
    });
  }
</script>

<main class="min-h-screen bg-gray-900 text-white pb-16">
  <header class="sticky top-0 z-40 bg-gray-900/80 backdrop-blur-md border-b border-white/5">
    <div class="flex flex-wrap items-center gap-4 px-4 sm:px-6 py-3">
      <div class="flex items-center gap-3 min-w-0">
        <div class="w-8 h-8 rounded-lg bg-blue-600 flex items-center justify-center shrink-0">
          <svg class="w-4 h-4 text-white" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <path d="M4 6h16v2H4zm0 5h16v2H4zm0 5h16v2H4z" />
          </svg>
        </div>
        <div class="min-w-0">
          <h1 class="text-sm font-semibold truncate">
            {session?.server_name ?? "jfFast"}
          </h1>
          <p class="text-xs text-gray-500 truncate">
            {session?.username ?? ""}
          </p>
        </div>
      </div>

      <nav class="flex items-center gap-1 sm:gap-2 bg-white/5 rounded-lg p-1" aria-label="Primary">
        <button
          type="button"
          onclick={() => navigateTo("/home")}
          class="px-3 py-1.5 text-xs sm:text-sm rounded-md transition-colors {isHomeRoute ? 'bg-blue-600 text-white' : 'text-gray-300 hover:text-white hover:bg-white/10'}"
          aria-current={isHomeRoute ? "page" : undefined}
        >
          Home
        </button>
        <button
          type="button"
          onclick={() => navigateTo("/library")}
          class="px-3 py-1.5 text-xs sm:text-sm rounded-md transition-colors {isLibraryRoute ? 'bg-blue-600 text-white' : 'text-gray-300 hover:text-white hover:bg-white/10'}"
          aria-current={isLibraryRoute ? "page" : undefined}
        >
          Library
        </button>
        <button
          type="button"
          onclick={() => navigateTo("/search")}
          class="px-3 py-1.5 text-xs sm:text-sm rounded-md transition-colors {isSearchRoute ? 'bg-blue-600 text-white' : 'text-gray-300 hover:text-white hover:bg-white/10'}"
          aria-current={isSearchRoute ? "page" : undefined}
        >
          Search
        </button>
        <button
          type="button"
          onclick={() => navigateTo("/settings")}
          class="px-3 py-1.5 text-xs sm:text-sm rounded-md transition-colors {isSettingsRoute ? 'bg-blue-600 text-white' : 'text-gray-300 hover:text-white hover:bg-white/10'}"
          aria-current={isSettingsRoute ? "page" : undefined}
        >
          Settings
        </button>
      </nav>

      <div class="flex-1 min-w-52 max-w-xl ml-auto">
        <div class="relative">
          <svg
            class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500"
            viewBox="0 0 20 20"
            fill="currentColor"
            aria-hidden="true"
          >
            <path
              fill-rule="evenodd"
              d="M8 4a4 4 0 100 8 4 4 0 000-8zM2 8a6 6 0 1110.89 3.476l4.817 4.817a1 1 0 01-1.414 1.414l-4.816-4.816A6 6 0 012 8z"
              clip-rule="evenodd"
            />
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

  {#if statusMessage}
    <div class="px-6 pt-3">
      <div
        class={`rounded-lg border px-3 py-2 text-sm ${
          (!online || staleData)
            ? "border-yellow-700 bg-yellow-900/35 text-yellow-200"
            : "border-orange-700 bg-orange-900/35 text-orange-200"
        }`}
      >
        {statusMessage}
      </div>
    </div>
  {/if}

  {#if isSearchRoute}
    <section class="px-6 pt-6" aria-label="Search results">
      {#if !searchQuery.trim()}
        <p class="text-gray-400 text-sm">Type in the search field to browse your media.</p>
      {:else if searching}
        <p class="text-gray-400 text-sm">Searching...</p>
      {:else if searchResults.length === 0}
        <p class="text-gray-400 text-sm">No results found.</p>
      {:else}
        <p class="text-gray-500 text-xs mb-4">
          {searchResults.length} results (from {searchSource === "remote" ? "server" : "local cache"})
        </p>
        <div class="space-y-8">
          {#if movieResults.length > 0}
            <section>
              <h2 class="text-sm font-semibold text-gray-200 mb-3">Movies ({movieResults.length})</h2>
              <div class="flex flex-wrap gap-3">
                {#each movieResults as item (item.id)}
                  <PosterCard {item} />
                {/each}
              </div>
            </section>
          {/if}

          {#if showResults.length > 0}
            <section>
              <h2 class="text-sm font-semibold text-gray-200 mb-3">Shows ({showResults.length})</h2>
              <div class="flex flex-wrap gap-3">
                {#each showResults as item (item.id)}
                  <PosterCard {item} />
                {/each}
              </div>
            </section>
          {/if}

          {#if episodeResults.length > 0}
            <section>
              <h2 class="text-sm font-semibold text-gray-200 mb-3">Episodes ({episodeResults.length})</h2>
              <div class="flex flex-wrap gap-3">
                {#each episodeResults as item (item.id)}
                  <PosterCard {item} />
                {/each}
              </div>
            </section>
          {/if}

          {#if otherResults.length > 0}
            <section>
              <h2 class="text-sm font-semibold text-gray-300 mb-3">Other ({otherResults.length})</h2>
              <div class="flex flex-wrap gap-3">
                {#each otherResults as item (item.id)}
                  <PosterCard {item} />
                {/each}
              </div>
            </section>
          {/if}
        </div>
      {/if}
    </section>
  {:else if isLibraryRoute}
    <section class="px-6 pt-6 pb-10" aria-label="Library browsing">
      <div class="flex flex-wrap items-center gap-3 mb-4">
        <h2 class="text-xl font-semibold">{selectedLibrary?.name ?? "Library"}</h2>
        <span class="text-xs text-gray-400 bg-white/5 rounded-full px-2 py-1">
          {filteredLibraryItems.length} items
        </span>
      </div>

      <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-5 gap-3 mb-6">
        <label class="text-xs text-gray-400 flex flex-col gap-1.5">
          View
          <select
            class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm text-white"
            value={selectedLibraryId}
            onchange={(event) => {
              const target = event.target as HTMLSelectElement;
              updateLibraryQuery({ view: target.value });
            }}
            aria-label="Select library"
          >
            {#each userLibraries as library (library.id)}
              <option value={library.id} class="bg-gray-900">{library.name}</option>
            {/each}
          </select>
        </label>

        <label class="text-xs text-gray-400 flex flex-col gap-1.5">
          Sort
          <select
            class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm text-white"
            value={selectedLibrarySort}
            onchange={(event) => {
              const target = event.target as HTMLSelectElement;
              updateLibraryQuery({ sort: target.value });
            }}
            aria-label="Sort library"
          >
            <option value="recent" class="bg-gray-900">Recently updated</option>
            <option value="title-asc" class="bg-gray-900">Title A-Z</option>
            <option value="title-desc" class="bg-gray-900">Title Z-A</option>
            <option value="rating-desc" class="bg-gray-900">Rating</option>
            <option value="year-desc" class="bg-gray-900">Year</option>
          </select>
        </label>

        <label class="text-xs text-gray-400 flex flex-col gap-1.5">
          Type
          <select
            class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm text-white"
            value={selectedLibraryTypeFilter}
            onchange={(event) => {
              const target = event.target as HTMLSelectElement;
              updateLibraryQuery({ type: target.value });
            }}
            aria-label="Filter by media type"
          >
            <option value="all" class="bg-gray-900">All media</option>
            <option value="movie" class="bg-gray-900">Movies</option>
            <option value="series" class="bg-gray-900">Series</option>
            <option value="season" class="bg-gray-900">Seasons</option>
            <option value="episode" class="bg-gray-900">Episodes</option>
          </select>
        </label>

        <label class="text-xs text-gray-400 flex flex-col gap-1.5">
          Status
          <select
            class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm text-white"
            value={selectedLibraryStatusFilter}
            onchange={(event) => {
              const target = event.target as HTMLSelectElement;
              updateLibraryQuery({ status: target.value });
            }}
            aria-label="Filter by watch status"
          >
            <option value="all" class="bg-gray-900">All statuses</option>
            <option value="unplayed" class="bg-gray-900">Unplayed</option>
            <option value="in-progress" class="bg-gray-900">In progress</option>
            <option value="favorites" class="bg-gray-900">Favorites</option>
          </select>
        </label>

        <div class="text-xs text-gray-400 flex flex-col gap-1.5">
          Layout
          <div class="inline-flex rounded-lg overflow-hidden border border-white/10 w-fit">
            <button
              type="button"
              onclick={() => updateLibraryQuery({ layout: "grid" })}
              class="px-3 py-2 text-sm transition-colors {selectedLibraryLayout === 'grid' ? 'bg-blue-600 text-white' : 'bg-white/8 text-gray-300 hover:text-white'}"
              aria-pressed={selectedLibraryLayout === "grid"}
            >
              Grid
            </button>
            <button
              type="button"
              onclick={() => updateLibraryQuery({ layout: "list" })}
              class="px-3 py-2 text-sm transition-colors {selectedLibraryLayout === 'list' ? 'bg-blue-600 text-white' : 'bg-white/8 text-gray-300 hover:text-white'}"
              aria-pressed={selectedLibraryLayout === "list"}
            >
              List
            </button>
          </div>
        </div>
      </div>

      {#if libraryLoading}
        <p class="text-gray-400 text-sm">Loading library...</p>
      {:else if libraryError}
        <div class="bg-red-500/10 border border-red-500/25 rounded-lg p-4 text-sm text-red-200">
          {libraryError}
        </div>
      {:else if filteredLibraryItems.length === 0}
        <p class="text-gray-400 text-sm">No items match your current filters.</p>
      {:else if selectedLibraryLayout === "grid"}
        <div class="flex flex-wrap gap-3">
          {#each filteredLibraryItems as item (item.id)}
            <PosterCard {item} />
          {/each}
        </div>
      {:else}
        <div class="space-y-2">
          {#each filteredLibraryItems as item (item.id)}
            <button
              type="button"
              onclick={() => push(`/item?id=${item.id}`)}
              class="w-full rounded-xl p-3 bg-white/5 hover:bg-white/10 transition-colors text-left"
              aria-label="Open {item.name} details"
            >
              <div class="flex items-start gap-3">
                <div class="w-16 h-24 rounded-lg overflow-hidden bg-gray-800 shrink-0">
                  {#if item.image_tag}
                    <img
                      src={`http://jfimage.localhost/poster/${item.id}?tag=${item.image_tag}`}
                      alt={item.name}
                      loading="lazy"
                      class="w-full h-full object-cover"
                    />
                  {:else}
                    <div class="w-full h-full flex items-center justify-center text-gray-500 text-xs px-1 text-center">
                      {item.name}
                    </div>
                  {/if}
                </div>
                <div class="min-w-0 flex-1">
                  <p class="text-sm text-white font-medium truncate">{item.name}</p>
                  <p class="text-xs text-gray-400 mt-1">
                    {item.type}
                    {#if item.production_year}
                      · {item.production_year}
                    {/if}
                    {#if item.run_time_ticks}
                      · {formatRuntime(item.run_time_ticks)}
                    {/if}
                  </p>
                </div>
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </section>
  {:else if isSettingsRoute}
    <section class="px-6 pt-6 pb-10 max-w-3xl" aria-label="Settings">
      <div class="flex items-center justify-between gap-3 mb-4">
        <h2 class="text-xl font-semibold">Preferences</h2>
        {#if preferencesSaving}
          <span class="text-xs text-blue-300">Saving...</span>
        {/if}
      </div>

      <div class="space-y-4">
        <div class="rounded-xl border border-white/10 bg-white/5 p-4">
          <h3 class="text-sm font-semibold mb-3">Playback</h3>
          <div class="grid gap-3 md:grid-cols-2">
            <label class="text-sm text-gray-200 flex items-center gap-2">
              <input
                type="checkbox"
                checked={preferences.playback.autoplay_next_episode}
                onchange={setAutoplayPreference}
              />
              Autoplay next episode
            </label>

            <label class="text-sm text-gray-300 flex flex-col gap-1">
              Default playback speed
              <input
                type="number"
                min="0.5"
                max="2"
                step="0.1"
                value={preferences.playback.default_playback_rate}
                onchange={setPlaybackRatePreference}
                class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm"
              />
            </label>
          </div>
        </div>

        <div class="rounded-xl border border-white/10 bg-white/5 p-4">
          <h3 class="text-sm font-semibold mb-3">Language + Quality</h3>
          <div class="grid gap-3 md:grid-cols-2">
            <label class="text-sm text-gray-300 flex flex-col gap-1">
              Preferred audio language
              <input
                type="text"
                value={preferences.language.preferred_audio_language}
                onchange={setAudioLanguagePreference}
                class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm"
                placeholder="en, cs, de..."
              />
            </label>

            <label class="text-sm text-gray-300 flex flex-col gap-1">
              Preferred subtitle language
              <input
                type="text"
                value={preferences.language.preferred_subtitle_language}
                onchange={setSubtitleLanguagePreference}
                class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm"
                placeholder="en, cs, de..."
              />
            </label>

            <label class="text-sm text-gray-300 flex flex-col gap-1 md:col-span-2">
              Default streaming quality
              <select
                class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm text-white"
                value={preferences.quality.default_quality_key}
                onchange={setQualityPreference}
              >
                <option value="direct-play" class="bg-gray-900">Direct Play</option>
                <option value="preset-1080" class="bg-gray-900">1080p</option>
                <option value="preset-720" class="bg-gray-900">720p</option>
                <option value="preset-480" class="bg-gray-900">480p</option>
              </select>
            </label>
          </div>
        </div>

        <div class="rounded-xl border border-white/10 bg-white/5 p-4">
          <h3 class="text-sm font-semibold mb-3">Cache + Refresh</h3>
          <div class="grid gap-3 md:grid-cols-2">
            <label class="text-sm text-gray-200 flex items-center gap-2 md:col-span-2">
              <input
                type="checkbox"
                checked={preferences.cache.enabled}
                onchange={setCacheEnabledPreference}
              />
              Enable local homepage cache
            </label>

            <label class="text-sm text-gray-300 flex flex-col gap-1">
              Cache max age (minutes)
              <input
                type="number"
                min="5"
                max="10080"
                value={preferences.cache.max_age_minutes}
                onchange={setCacheAgePreference}
                class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm"
                disabled={!preferences.cache.enabled}
              />
            </label>

            <label class="text-sm text-gray-300 flex flex-col gap-1">
              Refresh interval
              <select
                class="bg-white/8 border border-white/10 rounded-lg px-3 py-2 text-sm text-white"
                value={String(preferences.refresh_interval_seconds)}
                onchange={setRefreshIntervalPreference}
              >
                <option value="30" class="bg-gray-900">Every 30s</option>
                <option value="60" class="bg-gray-900">Every 1 min</option>
                <option value="180" class="bg-gray-900">Every 3 min</option>
                <option value="300" class="bg-gray-900">Every 5 min</option>
                <option value="600" class="bg-gray-900">Every 10 min</option>
              </select>
            </label>
          </div>
        </div>

        <div class="rounded-xl border border-white/10 bg-white/5 p-4">
          <h3 class="text-sm font-semibold mb-2">Maintenance</h3>
          <div class="flex flex-wrap gap-3">
            <Button variant="secondary" onclick={handleResync}>
              <span class="text-sm">{runningResync ? "Resyncing..." : "Force Resync"}</span>
            </Button>
            <Button variant="secondary" onclick={handleLogout}>
              <span class="text-sm">Log Out</span>
            </Button>
          </div>
        </div>
      </div>
    </section>
  {:else if loading && !hasCachedData}
    <div class="flex items-center justify-center h-64">
      <div class="text-center">
        <svg class="w-8 h-8 text-blue-400 animate-spin mx-auto mb-3" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25" />
          <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
        </svg>
        <p class="text-gray-400 text-sm">Loading your library...</p>
      </div>
    </div>
  {:else}
    <div class="space-y-2">
      <HeroBanner items={featuredItems} />

      <MediaRow title="Continue Watching" items={resumeItems} landscape={true} />
      <MediaRow title="Next Up" items={nextUpItems} landscape={true} />

      {#each userLibraries as library (library.id)}
        {#if libraryLatest[library.id]?.length}
          <section class="mb-6">
            <div class="flex items-center justify-between px-6 mb-2">
              <h2 class="text-lg font-semibold text-white">Latest in {library.name}</h2>
              <button
                type="button"
                onclick={() => openLibraryView(library)}
                class="text-sm text-blue-400 hover:text-blue-300 transition-colors"
                aria-label="View all in {library.name}"
              >
                View All
              </button>
            </div>

            <div class="flex gap-3 overflow-x-auto px-6 pb-4 scrollbar-hide">
              {#each libraryLatest[library.id] as item (item.id)}
                <PosterCard {item} />
              {/each}
            </div>
          </section>
        {/if}
      {/each}

      {#if !hasAnyContent()}
        <div class="flex flex-col items-center justify-center h-64 text-center px-6">
          <svg
            class="w-16 h-16 text-gray-700 mb-4"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            aria-hidden="true"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              d="M7 4v16M17 4v16M3 8h4m10 0h4M3 12h18M3 16h4m10 0h4M4 20h16a1 1 0 001-1V5a1 1 0 00-1-1H4a1 1 0 00-1 1v14a1 1 0 001 1z"
            />
          </svg>
          <p class="text-gray-400 text-lg font-medium mb-1">Your library is empty</p>
          <p class="text-gray-600 text-sm">Sync may still be in progress. Content will appear here once indexed.</p>
        </div>
      {/if}
    </div>
  {/if}
</main>

<SyncIndicator />
