<script lang="ts">
  import { onMount, tick } from "svelte";
  import { push } from "svelte-spa-router";
  import MediaRow from "../../components/media/MediaRow.svelte";
  import HeroCarousel from "../../components/media/HeroCarousel.svelte";
  import PosterCard from "../../components/media/PosterCard.svelte";
  import type { MediaItem, UserLibrary } from "../../lib/types";

  interface Props {
    loading: boolean;
    hasCachedData: boolean;
    resumeItems: MediaItem[];
    nextUpItems: MediaItem[];
    featuredItems: MediaItem[];
    userLibraries: UserLibrary[];
    libraryLatest: Record<string, MediaItem[]>;
  }

  let {
    loading,
    hasCachedData,
    resumeItems,
    nextUpItems,
    featuredItems,
    userLibraries,
    libraryLatest,
  }: Props = $props();

  let activeRouteHeading = $state<HTMLElement | null>(null);

  const carouselItems = $derived(
    nextUpItems.length > 0 
      ? nextUpItems.slice(0, 5) 
      : (resumeItems.length > 0 ? resumeItems.slice(0, 5) : featuredItems.slice(0, 5))
  );
  const activeCarouselIds = $derived(new Set(carouselItems.map(item => item.id)));

  const hasAnyContent = $derived(
    featuredItems.length > 0 ||
    resumeItems.length > 0 ||
    nextUpItems.length > 0 ||
    userLibraries.some(library => (libraryLatest[library.id]?.length ?? 0) > 0)
  );

  onMount(() => {
    void tick().then(() => {
      activeRouteHeading?.focus();
    });
  });

  function openLibraryView(library: UserLibrary) {
    push(
      `/library?view=${encodeURIComponent(library.id)}&layout=grid&sort=recent&type=all&status=all`,
    );
  }
</script>

{#if loading && !hasCachedData}
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

    {#if !hasAnyContent}
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
