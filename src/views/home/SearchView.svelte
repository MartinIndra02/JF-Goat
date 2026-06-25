<script lang="ts">
  import { onMount, tick } from "svelte";
  import { querystring } from "svelte-spa-router";
  import { searchItems } from "../../lib/api";
  import { pushErrorToast } from "../../lib/stores/toast.svelte";
  import PosterCard from "../../components/media/PosterCard.svelte";
  import type { MediaItem } from "../../lib/types";

  let activeRouteHeading = $state<HTMLElement | null>(null);

  const routeQuery = $derived(new URLSearchParams($querystring));
  const searchQuery = $derived((routeQuery.get("q") ?? "").trim());

  let searchResults = $state<MediaItem[]>([]);
  let searchSource = $state("");
  let searching = $state(false);
  let lastSearchTerm = "";
  let searchRequestId = 0;

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

  onMount(() => {
    void tick().then(() => {
      activeRouteHeading?.focus();
    });
  });

  $effect(() => {
    const term = searchQuery;
    if (!term) {
      lastSearchTerm = "";
      searching = false;
      searchResults = [];
      searchSource = "";
      return;
    }

    if (term !== lastSearchTerm) {
      void runSearch(term);
    }
  });

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
</script>

<section class="px-6 pt-6 pb-10 app-animate-fade-up" aria-label="Search results">
  <h2 bind:this={activeRouteHeading} tabindex="-1" class="sr-only">Search results</h2>
  {#if !searchQuery}
    <p class="app-muted text-sm">Type in the search field to browse your media.</p>
  {:else if searching}
    <p class="app-muted text-sm">Searching...</p>
  {:else if searchResults.length === 0}
    <p class="app-muted text-sm">No results found.</p>
  {:else}
    <p class="app-faint text-xs mb-4">
      {searchResults.length} results (from {searchSource === "remote" ? "remote Jellyfin API" : "local SQLite database"})
    </p>
    <div class="space-y-8">
      {#if movieResults.length > 0}
        <section class="glass-panel rounded-2xl p-4 sm:p-5">
          <h2 class="text-sm font-semibold text-[var(--text-primary)] mb-3">Movies ({movieResults.length})</h2>
          <div class="flex flex-wrap gap-3" role="list" aria-label="Movie results">
            {#each movieResults as item (item.id)}
              <div role="listitem">
                <PosterCard {item} />
              </div>
            {/each}
          </div>
        </section>
      {/if}

      {#if showResults.length > 0}
        <section class="glass-panel rounded-2xl p-4 sm:p-5">
          <h2 class="text-sm font-semibold text-[var(--text-primary)] mb-3">Shows ({showResults.length})</h2>
          <div class="flex flex-wrap gap-3" role="list" aria-label="Show results">
            {#each showResults as item (item.id)}
              <div role="listitem">
                <PosterCard {item} />
              </div>
            {/each}
          </div>
        </section>
      {/if}

      {#if episodeResults.length > 0}
        <section class="glass-panel rounded-2xl p-4 sm:p-5">
          <h2 class="text-sm font-semibold text-[var(--text-primary)] mb-3">Episodes ({episodeResults.length})</h2>
          <div class="flex flex-wrap gap-3" role="list" aria-label="Episode results">
            {#each episodeResults as item (item.id)}
              <div role="listitem">
                <PosterCard {item} />
              </div>
            {/each}
          </div>
        </section>
      {/if}

      {#if otherResults.length > 0}
        <section class="glass-panel rounded-2xl p-4 sm:p-5">
          <h2 class="text-sm font-semibold text-[var(--text-secondary)] mb-3">Other ({otherResults.length})</h2>
          <div class="flex flex-wrap gap-3" role="list" aria-label="Other results">
            {#each otherResults as item (item.id)}
              <div role="listitem">
                <PosterCard {item} />
              </div>
            {/each}
          </div>
        </section>
      {/if}
    </div>
  {/if}
</section>
