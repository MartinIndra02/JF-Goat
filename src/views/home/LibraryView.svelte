<script lang="ts">
  import { onMount, tick } from "svelte";
  import { location, querystring, replace, push } from "svelte-spa-router";
  import { getLibraryItems } from "../../lib/api";
  import { markDegraded } from "../../lib/stores/connectivity.svelte";
  import { pushToast } from "../../lib/stores/toast.svelte";
  import PosterCard from "../../components/media/PosterCard.svelte";
  import type { MediaItem } from "../../lib/types";

  import { homeDataStore } from "../../lib/stores/homeData.svelte";

  const userLibraries = $derived(homeDataStore.userLibraries);

  let activeRouteHeading = $state<HTMLElement | null>(null);

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

  const routeQuery = $derived(new URLSearchParams($querystring));
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


  onMount(() => {
    void tick().then(() => {
      activeRouteHeading?.focus();
    });

    const handleRefreshLibrary = async () => {
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

    window.addEventListener("refresh-library", handleRefreshLibrary);

    return () => {
      window.removeEventListener("refresh-library", handleRefreshLibrary);
    };
  });

  $effect(() => {
    if ($location !== "/library") return;
    if (selectedLibraryId) return;
    if (userLibraries.length === 0) return;

    replace(
      `/library?view=${encodeURIComponent(userLibraries[0].id)}&layout=grid&sort=recent&type=all&status=all`,
    );
  });

  $effect(() => {
    const viewId = selectedLibraryId;
    if (!viewId) return;
    if (viewId === lastLoadedLibraryId) return;
    void loadLibraryItems(viewId);
  });

  $effect(() => {
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
    if (libraryLoading || libraryLoadingMore) return;
    if (!libraryHasMore) return;
    if (filteredLibraryItems.length > 0) return;
    void loadMoreLibraryItems();
  });

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
      markDegraded(error);
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
      markDegraded(error);
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

  function updateLibraryQuery(changes: Record<string, string>) {
    const params = new URLSearchParams($querystring);
    for (const [key, value] of Object.entries(changes)) {
      params.set(key, value);
    }
    replace(`/library?${params.toString()}`);
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

  function formatRuntime(ticks: number | null): string {
    if (!ticks) return "";
    const minutes = Math.round(ticks / 600_000_000);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
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
</script>

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
