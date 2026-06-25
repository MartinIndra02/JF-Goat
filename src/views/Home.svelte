<script lang="ts">
  import { onMount, tick, untrack } from "svelte";

  import { location, push, querystring, replace } from "svelte-spa-router";
  import {
    getLatestItems,
    getLatestMedia,
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
  import LibraryView from "./home/LibraryView.svelte";
  import HomeDashboard from "./home/HomeDashboard.svelte";
  import SyncIndicator from "../components/layout/SyncIndicator.svelte";

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

    window.addEventListener("refresh-homepage", handleRefreshHomepage);

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






  async function handleSettingsResync() {
    await refreshFromServer();
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
    <LibraryView {userLibraries} />
  {:else if isOfflineRoute}
    <OfflineView />
  {:else if isSettingsRoute}
    <SettingsView onResync={handleSettingsResync} />
  {:else}
    <HomeDashboard
      {loading}
      {hasCachedData}
      {resumeItems}
      {nextUpItems}
      {featuredItems}
      {userLibraries}
      {libraryLatest}
    />
  {/if}
</main>

<SyncIndicator />
