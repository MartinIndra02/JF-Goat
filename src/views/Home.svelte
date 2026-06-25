<script lang="ts">
  import { onMount, tick, untrack } from "svelte";

  import { location, push, querystring, replace } from "svelte-spa-router";
  import {
    getLatestItems,
    getLatestMedia,
    getLibraryItems,
    getNextUp,
    getResumeItems,
    getUserViews,
    loadHomepageCache,
    saveHomepageCache,
    startSync,
  } from "../lib/api";
  import {
    filterSuppressedNextUp,
    HOMEPAGE_CACHE_UPDATED_EVENT,
  } from "../lib/homepageFreshness";
  import { getSession } from "../lib/stores/auth.svelte";
  import {
    isPreferencesLoaded,
    loadPreferences,
  } from "../lib/stores/preferences.svelte";

  import {
    getLastNetworkError,
    isDegraded,
    isOnline,
    markDegraded,
    markHealthy,
  } from "../lib/stores/connectivity.svelte";
  import { pushErrorToast, pushToast } from "../lib/stores/toast.svelte";
  import {
    initSyncListeners,
    isSyncTriggered,
    markSyncTriggered,
  } from "../lib/stores/sync.svelte";
  import SettingsView from "./home/SettingsView.svelte";
  import OfflineView from "./home/OfflineView.svelte";
  import SearchView from "./home/SearchView.svelte";
  import SyncIndicator from "../components/layout/SyncIndicator.svelte";
  import MediaRow from "../components/media/MediaRow.svelte";
  import HeroCarousel from "../components/media/HeroCarousel.svelte";
  import PosterCard from "../components/media/PosterCard.svelte";

  import TextInput from "../components/ui/TextInput.svelte";
  import type { HomepageCache, MediaItem, UserLibrary } from "../lib/types";

  const session = getSession();

  let searchQuery = $state("");
  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let searchInput = $state<HTMLInputElement | null>(null);

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
  let libraryLoadMoreError = $state("");
  let libraryLoadingMore = $state(false);
  let libraryHasMore = $state(false);
  let libraryPage = $state(1);
  let libraryTotalCount = $state<number | null>(null);
  let libraryScrollSentinel = $state<HTMLDivElement | null>(null);
  let lastLoadedLibraryId = "";
  let libraryRequestId = 0;

  const LIBRARY_PAGE_SIZE = 120;

  let refreshTimer: ReturnType<typeof setInterval> | null = null;
  let staleClockTimer: ReturnType<typeof setInterval> | null = null;
  let cacheFreshnessNow = $state(Date.now());
  let lastDataRefreshAt = $state<number | null>(null);
  let activeRouteHeading = $state<HTMLElement | null>(null);
  let lastFocusedRoute = "";

  const currentPath = $derived($location || "/home");
  const routeQuery = $derived(new URLSearchParams($querystring));

  const isHomeRoute = $derived(currentPath === "/home");
  const isLibraryRoute = $derived(currentPath === "/library");
  const isOfflineRoute = $derived(currentPath === "/offline");
  const isSearchRoute = $derived(currentPath === "/search");
  const isSettingsRoute = $derived(currentPath === "/settings");



  const selectedLibraryId = $derived(routeQuery.get("view") ?? "");
  const carouselItems = $derived(
    nextUpItems.length > 0 
      ? nextUpItems.slice(0, 5) 
      : (resumeItems.length > 0 ? resumeItems.slice(0, 5) : featuredItems.slice(0, 5))
  );
  const activeCarouselIds = $derived(new Set(carouselItems.map(item => item.id)));
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

  const preferencesLoaded = $derived(isPreferencesLoaded());

  const online = $derived(isOnline());
  const degraded = $derived(isDegraded());
  const lastNetworkError = $derived(getLastNetworkError());
  const staleAgeMs = $derived(lastDataRefreshAt ? cacheFreshnessNow - lastDataRefreshAt : null);
  const staleData = $derived(staleAgeMs !== null && staleAgeMs > 180000);

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
  if (!isSyncTriggered()) {
    markSyncTriggered();
    startSync().catch((error) => {
      pushErrorToast("sync", error, "Sync startup failed", "sync-start-failed");
    });
  }

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

    const handleRefreshHomepage = async () => {
      if (!isHomeRoute) return;
      pushToast({
        level: "info",
        source: "api",
        title: "Refreshing dashboard",
        message: "Fetching updated media from server...",
        dismissAfterMs: 3000,
      });
      await refreshFromServer();
      pushToast({
        level: "success",
        source: "api",
        title: "Dashboard updated",
        message: "Your home screen is now up to date.",
        dismissAfterMs: 3000,
      });
    };

    const handleRefreshLibrary = async () => {
      if (!isLibraryRoute) return;
      const viewId = selectedLibraryId;
      if (!viewId) return;
      pushToast({
        level: "info",
        source: "api",
        title: "Refreshing library",
        message: "Fetching the latest library content...",
        dismissAfterMs: 3000,
      });
      await loadLibraryItems(viewId);
      pushToast({
        level: "success",
        source: "api",
        title: "Library updated",
        message: "The library view is now up to date.",
        dismissAfterMs: 3000,
      });
    };

    window.addEventListener("refresh-homepage", handleRefreshHomepage);
    window.addEventListener("refresh-library", handleRefreshLibrary);

    staleClockTimer = setInterval(() => {
      cacheFreshnessNow = Date.now();
    }, 30_000);

    void initializeHome();

    return () => {
      window.removeEventListener(
        HOMEPAGE_CACHE_UPDATED_EVENT,
        onHomepageCacheUpdated as EventListener,
      );

      window.removeEventListener("refresh-homepage", handleRefreshHomepage);
      window.removeEventListener("refresh-library", handleRefreshLibrary);

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
  }

  $effect(() => {
    const routePath = currentPath;
    if (routePath === lastFocusedRoute) return;

    lastFocusedRoute = routePath;
    void tick().then(() => {
      if (routePath === "/search") {
        searchInput?.focus();
      } else {
        activeRouteHeading?.focus();
      }
    });
  });

  $effect(() => {
    if (!isSearchRoute) return;
    const routeSearch = (routeQuery.get("q") ?? "").trim();

    if (untrack(() => searchQuery) !== routeSearch) {
      searchQuery = routeSearch;
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
    if (viewId === lastLoadedLibraryId) return;
    void loadLibraryItems(viewId);
  });

  $effect(() => {
    if (!isLibraryRoute) return;
    if (!libraryScrollSentinel) return;
    if (libraryLoading) return;
    if (!libraryHasMore) return;

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          void loadMoreLibraryItems();
        }
      },
      {
        root: null,
        rootMargin: "700px 0px 700px 0px",
        threshold: 0,
      },
    );

    observer.observe(libraryScrollSentinel);

    return () => {
      observer.disconnect();
    };
  });

  $effect(() => {
    if (!isLibraryRoute) return;
    if (libraryLoading || libraryLoadingMore) return;
    if (!libraryHasMore) return;
    if (filteredLibraryItems.length > 0) return;
    void loadMoreLibraryItems();
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

      await saveHomepageCache({
        resume_items: resume,
        next_up_items: filteredNextUp,
        user_libraries: views,
        library_latest: latestMap,
        featured_items: featured,
        cache_refreshed_at_epoch_ms: refreshedAt,
      });

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
    const requestId = ++libraryRequestId;
    libraryLoading = true;
    libraryLoadingMore = false;
    libraryError = "";
    libraryLoadMoreError = "";
    libraryItems = [];
    libraryPage = 1;
    libraryHasMore = false;
    libraryTotalCount = null;
    lastLoadedLibraryId = "";

    try {
      const result = await getLibraryItems(viewId, 1, LIBRARY_PAGE_SIZE);
      if (requestId !== libraryRequestId) return;

      libraryItems = dedupeAndAppendLibraryItems([], result.items);
      libraryPage = 1;
      libraryHasMore = result.has_more;
      libraryTotalCount = result.total_record_count;
      lastLoadedLibraryId = viewId;
    } catch (error) {
      if (requestId !== libraryRequestId) return;
      markDegraded(String(error));
      libraryError = "Could not load this library. Check your connection and try again.";
      libraryItems = [];
      lastLoadedLibraryId = "";
    } finally {
      if (requestId === libraryRequestId) {
        libraryLoading = false;
      }
    }
  }

  async function loadMoreLibraryItems() {
    const viewId = selectedLibraryId;
    if (!viewId) return;
    if (libraryLoading || libraryLoadingMore) return;
    if (!libraryHasMore) return;

    const requestId = libraryRequestId;
    const nextPage = libraryPage + 1;

    libraryLoadingMore = true;
    libraryLoadMoreError = "";

    try {
      const result = await getLibraryItems(viewId, nextPage, LIBRARY_PAGE_SIZE);
      if (requestId !== libraryRequestId) return;

      libraryItems = dedupeAndAppendLibraryItems(libraryItems, result.items);
      libraryPage = nextPage;
      libraryHasMore = result.has_more;

      if (nextPage === 1 || libraryTotalCount === null) {
        libraryTotalCount = result.total_record_count;
      }
    } catch (error) {
      if (requestId !== libraryRequestId) return;
      markDegraded(String(error));
      libraryLoadMoreError = "Could not load more items. Scroll to retry.";
    } finally {
      if (requestId === libraryRequestId) {
        libraryLoadingMore = false;
      }
    }
  }

  function dedupeAndAppendLibraryItems(
    existing: MediaItem[],
    incoming: MediaItem[],
  ): MediaItem[] {
    if (incoming.length === 0) return existing;

    const seenIds = new Set(existing.map((item) => item.id));
    const merged = [...existing];

    for (const item of incoming) {
      if (seenIds.has(item.id)) continue;
      seenIds.add(item.id);
      merged.push(item);
    }

    return merged;
  }



  function triggerSearchImmediately() {
    if (searchTimer) {
      clearTimeout(searchTimer);
      searchTimer = null;
    }

    const trimmed = searchQuery.trim();
    if (!trimmed) {
      if (isSearchRoute) {
        replace("/search");
      }
      return;
    }

    const target = `/search?q=${encodeURIComponent(trimmed)}`;
    if (isSearchRoute) {
      replace(target);
    } else {
      push(target);
    }
  }

  function onSearchInput() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(triggerSearchImmediately, 250);
  }

  function onSearchKeyDown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      triggerSearchImmediately();
    }
  }

  function navigateTo(path: string) {
    if (currentPath === path) return;
    push(path);
  }

  function openItem(item: MediaItem) {
    if (item.type === "BoxSet") {
      push(
        `/library?view=${encodeURIComponent(item.id)}&layout=grid&sort=recent&type=all&status=all`,
      );
      return;
    }

    push(`/item?id=${encodeURIComponent(item.id)}`);
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

  async function handleSettingsResync() {
    await refreshFromServer();
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


</script>

<main class="min-h-screen text-[var(--text-primary)] pb-16">
  <header class="sticky top-0 z-40 border-b border-white/10 bg-[rgba(6,10,18,0.55)] backdrop-blur-xl">
    <div class="flex flex-wrap items-center justify-between gap-4 px-4 sm:px-6 py-3.5">
      
      <div class="flex items-center gap-4 min-w-0">
        <button
          type="button"
          onclick={() => navigateTo("/home")}
          class="group flex items-center gap-3 text-left focus:outline-none focus-visible:ring-2 focus-visible:ring-[#66d8ff] rounded-xl px-2.5 py-1.5 -ml-2.5 hover:bg-white/5 transition-all duration-200"
        >
          <div class="w-9 h-9 rounded-xl bg-[linear-gradient(140deg,rgba(102,216,255,0.86),rgba(65,184,213,0.78))] group-hover:scale-105 group-hover:brightness-110 transition-all duration-200 flex items-center justify-center shrink-0 text-slate-950 shadow-[0_10px_24px_rgba(65,184,213,0.35)]">
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 3L4 9v11a1 1 0 001 1h5v-6h4v6h5a1 1 0 001-1V9l-8-6z"/>
            </svg>
          </div>
          <div class="min-w-0 hidden sm:block">
            <h1 class="text-sm font-semibold truncate tracking-[0.02em] text-white group-hover:text-cyan-300 transition-colors">
              {session?.server_name ?? "jfFast"}
            </h1>
            <p class="text-xs app-faint truncate">
              {session?.username ?? ""}
            </p>
          </div>
        </button>

        <nav class="flex items-center gap-1" aria-label="Primary navigation">
          <button
            type="button"
            onclick={() => navigateTo("/library")}
            class="px-3.5 py-2 rounded-xl text-xs font-semibold tracking-[0.02em] transition-all duration-200 border {isLibraryRoute ? 'bg-cyan-500/10 border-cyan-400/20 text-cyan-300' : 'border-transparent text-[var(--text-secondary)] hover:text-white hover:bg-white/5'}"
            aria-current={isLibraryRoute ? "page" : undefined}
          >
            Library
          </button>
          <button
            type="button"
            onclick={() => navigateTo("/offline")}
            class="px-3.5 py-2 rounded-xl text-xs font-semibold tracking-[0.02em] transition-all duration-200 border {isOfflineRoute ? 'bg-cyan-500/10 border-cyan-400/20 text-cyan-300' : 'border-transparent text-[var(--text-secondary)] hover:text-white hover:bg-white/5'}"
            aria-current={isOfflineRoute ? "page" : undefined}
          >
            Offline
          </button>
        </nav>
      </div>

      <div class="flex items-center gap-3 ml-auto flex-1 max-w-sm justify-end">
        <div class="relative w-full max-w-xs">
          <svg
            class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 app-faint"
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
            bind:element={searchInput}
            bind:value={searchQuery}
            placeholder="Search your library..."
            oninput={onSearchInput}
            onkeydown={onSearchKeyDown}
            inputClass="pl-10"
          />
        </div>

        <button
          type="button"
          onclick={() => navigateTo("/settings")}
          aria-label="Settings"
          class="group w-9 h-9 rounded-xl flex items-center justify-center shrink-0 border {isSettingsRoute ? 'bg-cyan-500/10 border-cyan-400/20 text-cyan-300 shadow-[0_4px_12px_rgba(65,184,213,0.15)]' : 'border-white/10 bg-white/5 text-[var(--text-secondary)] hover:text-white hover:bg-white/10 hover:border-white/20'} transition-all duration-200"
        >
          <svg class="w-4 h-4 transition-transform duration-500 group-hover:rotate-45" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.43l-1.003.828c-.293.241-.438.613-.43.992a7.723 7.723 0 010 .255c-.008.378.137.75.43.99l1.005.831a1.125 1.125 0 01.26 1.43l-1.297 2.247a1.125 1.125 0 01-1.37.491l-1.216-.456c-.356-.133-.751-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.43l1.004-.83c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.831a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.645-.869l.214-1.28z" />
            <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
      </div>
     </div>
   </header>

  {#if statusMessage}
    <div class="px-6 pt-3">
      <div
        class={`rounded-xl border px-3.5 py-2.5 text-sm backdrop-blur-md ${
          (!online || staleData)
            ? "border-amber-300/30 bg-amber-500/12 text-amber-100"
            : "border-orange-300/30 bg-orange-500/12 text-orange-100"
        }`}
      >
        {statusMessage}
      </div>
    </div>
  {/if}

  {#if isSearchRoute}
    <SearchView />
  {:else if isLibraryRoute}
    <section class="px-6 pt-6 pb-10 app-animate-fade-up" aria-label="Library browsing">
      <div class="flex flex-wrap items-center gap-3 mb-4">
        <h2 bind:this={activeRouteHeading} tabindex="-1" class="text-xl font-semibold">{selectedLibrary?.name ?? "Library"}</h2>
        <span class="text-xs app-pill px-2.5 py-1">
          {filteredLibraryItems.length} shown
          {#if libraryTotalCount !== null}
            · {libraryItems.length}/{libraryTotalCount} loaded
          {/if}
        </span>
      </div>

      <div class="glass-panel rounded-2xl p-4 sm:p-5 mb-6">
        <div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-5 gap-3">
        <label class="text-xs app-faint flex flex-col gap-1.5">
          View
          <select
            class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
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

        <label class="text-xs app-faint flex flex-col gap-1.5">
          Sort
          <select
            class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
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

        <label class="text-xs app-faint flex flex-col gap-1.5">
          Type
          <select
            class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
            value={selectedLibraryTypeFilter}
            onchange={(event) => {
              const target = event.target as HTMLSelectElement;
              updateLibraryQuery({ type: target.value });
            }}
            aria-label="Filter by media type"
          >
            <option value="all" class="bg-gray-900">All media</option>
            <option value="movie" class="bg-gray-900">Movies</option>
            <option value="series" class="bg-gray-900">Shows</option>
            <option value="season" class="bg-gray-900">Seasons</option>
            <option value="episode" class="bg-gray-900">Episodes</option>
          </select>
        </label>

        <label class="text-xs app-faint flex flex-col gap-1.5">
          Status
          <select
            class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
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

        <div class="text-xs app-faint flex flex-col gap-1.5">
          Layout
          <div class="inline-flex rounded-xl overflow-hidden border border-white/14 w-fit">
            <button
              type="button"
              onclick={() => updateLibraryQuery({ layout: "grid" })}
              class="px-3 py-2 text-sm transition-colors {selectedLibraryLayout === 'grid' ? 'bg-[rgba(102,216,255,0.28)] text-cyan-100' : 'bg-[rgba(13,21,35,0.72)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]'}"
              aria-pressed={selectedLibraryLayout === "grid"}
            >
              Grid
            </button>
            <button
              type="button"
              onclick={() => updateLibraryQuery({ layout: "list" })}
              class="px-3 py-2 text-sm transition-colors {selectedLibraryLayout === 'list' ? 'bg-[rgba(102,216,255,0.28)] text-cyan-100' : 'bg-[rgba(13,21,35,0.72)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]'}"
              aria-pressed={selectedLibraryLayout === "list"}
            >
              List
            </button>
          </div>
        </div>
        </div>
      </div>

      {#if libraryLoading}
        <p class="app-muted text-sm">Loading library...</p>
      {:else if libraryError}
        <div class="rounded-xl border border-red-300/30 bg-red-500/12 p-4 text-sm text-red-100">
          {libraryError}
        </div>
      {:else}
        {#if filteredLibraryItems.length === 0 && !libraryHasMore}
          <p class="app-muted text-sm">No items match your current filters.</p>
        {:else if selectedLibraryLayout === "grid"}
          <div
            class="flex flex-wrap gap-3"
            role="list"
            aria-label="{selectedLibrary?.name ?? 'Library'} poster grid"
          >
            {#each filteredLibraryItems as item (item.id)}
              <div role="listitem">
                <PosterCard {item} />
              </div>
            {/each}
          </div>
        {:else}
          <div class="space-y-2">
            {#each filteredLibraryItems as item (item.id)}
              <button
                type="button"
                onclick={() => openItem(item)}
                class="w-full rounded-2xl p-3 border border-white/12 bg-[rgba(255,255,255,0.04)] hover:bg-[rgba(255,255,255,0.09)] transition-colors text-left"
                aria-label="Open {item.name} details"
              >
                <div class="flex items-start gap-3">
                  <div class="w-16 h-24 rounded-lg overflow-hidden bg-[rgba(8,13,24,0.84)] shrink-0 border border-white/10">
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
                    <p class="text-sm text-[var(--text-primary)] font-medium truncate">{item.name}</p>
                    <p class="text-xs app-muted mt-1">
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

        {#if filteredLibraryItems.length === 0 && libraryHasMore && !libraryLoadingMore}
          <p class="app-faint text-sm mt-4">Scanning additional pages for matching items...</p>
        {/if}

        {#if libraryLoadMoreError}
          <p class="text-red-200 text-sm mt-4">{libraryLoadMoreError}</p>
        {/if}

        {#if libraryLoadingMore}
          <p class="app-muted text-sm mt-4">Loading more items...</p>
        {/if}

        {#if libraryHasMore}
          <div bind:this={libraryScrollSentinel} class="h-1 w-full" aria-hidden="true"></div>
        {/if}
      {/if}
    </section>
  {:else if isOfflineRoute}
    <OfflineView />
  {:else if isSettingsRoute}
    <SettingsView onResync={handleSettingsResync} />
  {:else if loading && !hasCachedData}
    <div class="flex items-center justify-center h-64">
      <div class="text-center">
        <svg class="w-8 h-8 text-blue-400 animate-spin mx-auto mb-3" viewBox="0 0 24 24" fill="none" aria-hidden="true">
          <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25" />
          <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
        </svg>
        <p class="app-muted text-sm">Loading your library...</p>
      </div>
    </div>
  {:else}
    <div class="space-y-6 app-animate-fade-up">
      <h2 bind:this={activeRouteHeading} tabindex="-1" class="sr-only">Home</h2>

      {#if carouselItems.length > 0}
        <div class="px-6 pt-2">
          <HeroCarousel items={carouselItems} />
        </div>
      {/if}

      <MediaRow 
        title="Continue Watching" 
        items={resumeItems.filter(item => !activeCarouselIds.has(item.id))} 
        landscape={true} 
      />
      <MediaRow 
        title="Next Up" 
        items={nextUpItems} 
        landscape={true} 
      />

      {#each userLibraries as library (library.id)}
        {#if libraryLatest[library.id]?.length}
          <section class="mb-6">
            <div class="flex items-center justify-between px-6 mb-2">
              <h2 class="text-lg font-semibold text-[var(--text-primary)]">Latest in {library.name}</h2>
              <button
                type="button"
                onclick={() => openLibraryView(library)}
                class="text-sm text-cyan-200 hover:text-cyan-100 transition-colors"
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
          <p class="app-muted text-lg font-medium mb-1">Your library is empty</p>
          <p class="app-faint text-sm">Sync may still be in progress. Content will appear here once indexed.</p>
        </div>
      {/if}
    </div>
  {/if}
</main>

<SyncIndicator />
