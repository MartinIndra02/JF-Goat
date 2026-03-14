<script lang="ts">
  import type { MediaItem } from "../../lib/types";

  let {
    items,
  }: {
    items: MediaItem[];
  } = $props();

  let currentIndex = $state(0);
  let intervalId: ReturnType<typeof setInterval> | null = null;

  const current = $derived(items[currentIndex]);

  function startAutoPlay() {
    stopAutoPlay();
    if (items.length > 1) {
      intervalId = setInterval(() => {
        currentIndex = (currentIndex + 1) % items.length;
      }, 8000);
    }
  }

  function stopAutoPlay() {
    if (intervalId) {
      clearInterval(intervalId);
      intervalId = null;
    }
  }

  function goTo(index: number) {
    currentIndex = index;
    startAutoPlay();
  }

  function formatRuntime(ticks: number | null): string {
    if (!ticks) return "";
    const minutes = Math.round(ticks / 600_000_000);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  }

  $effect(() => {
    if (items.length > 0) {
      startAutoPlay();
    }
    return () => stopAutoPlay();
  });
</script>

{#if items.length > 0 && current}
  <div class="relative w-full overflow-hidden" style="height: clamp(240px, 40vh, 420px);">
    <!-- Backdrop image -->
    {#if current.backdrop_tag}
      {#key current.id}
        <img
          src={`http://jfimage.localhost/backdrop/${current.id}?tag=${current.backdrop_tag}`}
          alt={current.name}
          class="absolute inset-0 w-full h-full object-cover animate-fade-in"
        />
      {/key}
    {:else if current.image_tag}
      {#key current.id}
        <div class="absolute inset-0 bg-gray-800 animate-fade-in"></div>
      {/key}
    {:else}
      <div class="absolute inset-0 bg-gray-800"></div>
    {/if}

    <!-- Gradient overlays -->
    <div class="absolute inset-0 bg-gradient-to-t from-gray-900 via-gray-900/40 to-transparent"></div>
    <div class="absolute inset-0 bg-gradient-to-r from-gray-900/80 via-transparent to-transparent"></div>

    <!-- Content -->
    <div class="absolute bottom-0 left-0 right-0 p-6 pb-8">
      <div class="max-w-xl">
        {#key current.id}
          <div class="animate-fade-in">
            <div class="flex items-center gap-2 mb-2">
              <span class="text-xs font-medium text-blue-400 bg-blue-400/10 px-2 py-0.5 rounded">
                {current.type}
              </span>
              {#if current.production_year}
                <span class="text-xs text-gray-400">{current.production_year}</span>
              {/if}
              {#if current.community_rating}
                <span class="flex items-center gap-1 text-xs text-gray-400">
                  <svg class="w-3 h-3 text-yellow-500" viewBox="0 0 20 20" fill="currentColor">
                    <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"/>
                  </svg>
                  {current.community_rating.toFixed(1)}
                </span>
              {/if}
              {#if current.run_time_ticks}
                <span class="text-xs text-gray-400">{formatRuntime(current.run_time_ticks)}</span>
              {/if}
            </div>
            <h1 class="text-2xl sm:text-3xl font-bold text-white mb-2 line-clamp-1">{current.name}</h1>
            {#if current.overview}
              <p class="text-sm text-gray-300 line-clamp-2 max-w-lg">{current.overview}</p>
            {/if}
            {#if current.genres}
              <div class="flex gap-2 mt-2">
                {#each current.genres.split(",").slice(0, 3) as genre}
                  <span class="text-xs text-gray-400 bg-white/10 px-2 py-0.5 rounded-full">
                    {genre.trim()}
                  </span>
                {/each}
              </div>
            {/if}
          </div>
        {/key}
      </div>
    </div>

    <!-- Navigation dots -->
    {#if items.length > 1}
      <div class="absolute bottom-3 right-6 flex gap-1.5">
        {#each items as _, i}
          <button
            onclick={() => goTo(i)}
            class="w-2 h-2 rounded-full transition-all duration-300 {i === currentIndex ? 'bg-white w-5' : 'bg-white/40 hover:bg-white/60'}"
            aria-label="Go to slide {i + 1}"
          ></button>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  :global(.animate-fade-in) {
    animation: fadeIn 0.5s ease-out;
  }
</style>
