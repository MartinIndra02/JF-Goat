<script lang="ts">
  import { logout as logoutApi, startSync, searchItems } from "../lib/api";
  import { getSession, setUnauthenticated } from "../lib/stores/auth.svelte";
  import { initSyncListeners } from "../lib/stores/sync.svelte";
  import Button from "../components/ui/Button.svelte";
  import TextInput from "../components/ui/TextInput.svelte";
  import SyncIndicator from "../components/layout/SyncIndicator.svelte";
  import { push } from "svelte-spa-router";
  import type { MediaItem } from "../lib/types";

  const session = getSession();

  let searchQuery = $state("");
  let searchResults = $state<MediaItem[]>([]);
  let searchSource = $state<string>("");
  let searching = $state(false);
  let searchTimer: ReturnType<typeof setTimeout> | null = null;

  // Initialize sync listeners and trigger background sync
  initSyncListeners();
  startSync().catch((e) => console.error("Failed to start sync:", e));

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
    setUnauthenticated();
    push("/connect");
  }
</script>

<main class="min-h-screen bg-gray-900 text-white pb-16">
  <!-- Header -->
  <header class="flex items-center justify-between px-6 py-4 border-b border-gray-800">
    <div>
      <h1 class="text-xl font-bold">
        Welcome{session?.username ? `, ${session.username}` : ""}
      </h1>
      <p class="text-gray-500 text-xs">
        {#if session?.server_name}
          Connected to {session.server_name}
        {:else}
          Connected to Jellyfin
        {/if}
      </p>
    </div>
    <Button variant="danger" onclick={handleLogout}>
      Log Out
    </Button>
  </header>

  <!-- Search -->
  <div class="px-6 py-4">
    <TextInput
      bind:value={searchQuery}
      placeholder="Search your library..."
      oninput={onSearchInput}
    />
  </div>

  <!-- Search Results -->
  {#if searchQuery.trim()}
    <div class="px-6">
      {#if searching}
        <p class="text-gray-400 text-sm">Searching...</p>
      {:else if searchResults.length === 0}
        <p class="text-gray-400 text-sm">No results found.</p>
      {:else}
        <p class="text-gray-500 text-xs mb-3">
          {searchResults.length} results (from {searchSource === "remote" ? "server" : "local cache"})
        </p>
        <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4">
          {#each searchResults as item (item.id)}
            <div class="bg-gray-800 rounded-lg overflow-hidden hover:ring-2 hover:ring-blue-500 transition-all">
              <div class="aspect-[2/3] bg-gray-700 flex items-center justify-center">
                <span class="text-gray-500 text-xs px-2 text-center">{item.type}</span>
              </div>
              <div class="p-2">
                <p class="text-sm font-medium truncate">{item.name}</p>
                {#if item.production_year}
                  <p class="text-xs text-gray-400">{item.production_year}</p>
                {/if}
                {#if item.series_name}
                  <p class="text-xs text-gray-500 truncate">{item.series_name}</p>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</main>

<SyncIndicator />
