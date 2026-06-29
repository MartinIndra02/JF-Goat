<script lang="ts">
  import { onMount, tick } from "svelte";
  import { push } from "svelte-spa-router";
  import MediaRow from "../../components/media/MediaRow.svelte";
  import HeroCarousel from "../../components/media/HeroCarousel.svelte";
  import PosterCard from "../../components/media/PosterCard.svelte";
  import type { UserLibrary } from "../../lib/types";
  import { homeDataStore } from "../../lib/stores/homeData.svelte";

  let activeRouteHeading = $state<HTMLElement | null>(null);

  const resumeItems = $derived(homeDataStore.resumeItems);
  const nextUpItems = $derived(homeDataStore.nextUpItems);
  const featuredItems = $derived(homeDataStore.featuredItems);
  const userLibraries = $derived(homeDataStore.userLibraries);
  const libraryLatest = $derived(homeDataStore.libraryLatest);
  const loading = $derived(homeDataStore.loading);
  const hasCachedData = $derived(homeDataStore.hasCachedData);

  const carouselItems = $derived(
    nextUpItems.length > 0 
      ? nextUpItems.slice(0, 5) 
      : (resumeItems.length > 0 ? resumeItems.slice(0, 5) : featuredItems.slice(0, 5))
  );
  const activeCarouselIds = $derived(new Set(carouselItems.map(item => item.id)));

  onMount(() => {
    void tick().then(() => {
      activeRouteHeading?.focus();
    });
    void homeDataStore.initializeHome();
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
          <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6 xl:grid-cols-8 gap-4 px-6">
            {#each libraryLatest[library.id] as item (item.id)}
              <PosterCard {item} aspect="poster" />
            {/each}
          </div>
        </section>
      {/if}
    {/each}
  </div>
{/if}
