<script lang="ts">
  import { push } from "svelte-spa-router";
  import type { MediaItem } from "../../lib/types";
  import { showPlayer } from "../../lib/stores/player.svelte";
  import { mpvPlay } from "../../lib/api";
  import { pushErrorToast } from "../../lib/stores/toast.svelte";

  let {
    items,
  }: {
    items: MediaItem[];
  } = $props();

  let currentIndex = $state(0);
  let intervalId: ReturnType<typeof setInterval> | null = null;

  const current = $derived(items[currentIndex]);
  const backdropUrl = $derived(current ? getBackdropUrl(current) : null);

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

  function navigateToItem() {
    if (current) {
      push(`/item?id=${current.id}`);
    }
  }

  async function playItem(event: MouseEvent) {
    event.stopPropagation();
    if (!current) return;
    showPlayer(current.id, current.name);
    try {
      await mpvPlay({
        itemId: current.id,
        startTicks: current.playback_ticks || 0,
      });
    } catch (error) {
      pushErrorToast("player", error, "Failed to play item", "play-failed");
    }
  }

  function formatRuntime(ticks: number | null): string {
    if (!ticks) return "";
    const minutes = Math.round(ticks / 600_000_000);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  }

  // Determine the backdrop image URL.
  // Next Up items are episodes, so if series_id is present, we use the show's main banner (series backdrop).
  function getBackdropUrl(item: MediaItem): string | null {
    if (item.type === "Episode" && item.series_id) {
      return `http://jfimage.localhost/backdrop/${item.series_id}?tag=${item.series_id}`;
    }
    if (item.backdrop_tag) {
      return `http://jfimage.localhost/backdrop/${item.id}?tag=${item.backdrop_tag}`;
    }
    return null;
  }

  $effect(() => {
    if (items.length > 0) {
      startAutoPlay();
    }
    return () => stopAutoPlay();
  });
</script>

{#if items.length > 0 && current}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div 
    onclick={navigateToItem}
    onmouseenter={stopAutoPlay}
    onmouseleave={startAutoPlay}
    class="relative w-full overflow-hidden rounded-2xl border border-white/10 bg-slate-950/40 hover:border-white/18 transition-all duration-300 group cursor-pointer shadow-2xl"
    style="height: clamp(280px, 45vh, 460px);"
  >
    <!-- Background Image container with crossfade key -->
    <div class="absolute inset-0 w-full h-full overflow-hidden select-none pointer-events-none">
      {#key current.id}
        {#if backdropUrl}
          <img
            src={backdropUrl}
            alt={current.name}
            class="w-full h-full object-cover transition-all duration-1000 scale-102 group-hover:scale-105 opacity-0 animate-fade-in filter brightness-[0.68] contrast-[1.05]"
            onload={(e) => {
              const img = e.target as HTMLImageElement;
              img.classList.remove("opacity-0");
              img.classList.add("opacity-100");
            }}
          />
        {:else}
          <div class="w-full h-full bg-slate-900 animate-fade-in"></div>
        {/if}
      {/key}
    </div>

    <!-- Ambient gradients -->
    <div class="absolute inset-0 bg-radial-gradient-ambient pointer-events-none"></div>
    <div class="absolute inset-0 bg-gradient-to-t from-slate-950 via-slate-950/50 to-transparent pointer-events-none"></div>
    <div class="absolute inset-0 bg-gradient-to-r from-slate-950/85 via-transparent to-transparent pointer-events-none"></div>

    <!-- Spotlight / Next Up Tag -->
    <div class="absolute top-4 left-4 sm:top-6 sm:left-6 flex gap-2 pointer-events-none z-10">
      <span class="px-3 py-1 rounded-full text-[10px] font-bold uppercase tracking-wider bg-cyan-500/25 border border-cyan-300/30 text-cyan-200 backdrop-blur-md shadow-lg">
        Next Up
      </span>
    </div>

    <!-- Content Layout -->
    <div class="absolute inset-x-0 bottom-0 p-6 sm:p-8 flex flex-col justify-end h-full z-10">
      <div class="max-w-2xl space-y-3">
        
        {#key current.id}
          <div class="space-y-2 animate-fade-in-up">
            <!-- Show Title & Info -->
            <div class="flex flex-wrap items-center gap-2 sm:gap-3 text-xs text-slate-300">
              {#if current.series_name}
                <span class="font-bold text-cyan-400 uppercase tracking-wider">
                  {current.series_name}
                </span>
                <span class="w-1 h-1 rounded-full bg-slate-600"></span>
              {/if}
              {#if current.season_name}
                <span>{current.season_name}</span>
                <span class="w-1 h-1 rounded-full bg-slate-600"></span>
              {/if}
              {#if current.index_number !== null}
                <span>Episode {current.index_number}</span>
                <span class="w-1 h-1 rounded-full bg-slate-600"></span>
              {/if}
              {#if current.production_year}
                <span>{current.production_year}</span>
                <span class="w-1 h-1 rounded-full bg-slate-600"></span>
              {/if}
              {#if current.run_time_ticks}
                <span>{formatRuntime(current.run_time_ticks)}</span>
              {/if}
            </div>

            <!-- Episode Name -->
            <h1 class="text-3xl sm:text-4xl md:text-5xl font-black text-white tracking-tight leading-none drop-shadow-md select-text">
              {current.name}
            </h1>

            <!-- Episode Overview -->
            {#if current.overview}
              <p class="text-sm sm:text-base text-slate-200/90 leading-relaxed line-clamp-2 max-w-xl select-text drop-shadow-sm font-light">
                {current.overview}
              </p>
            {/if}
          </div>
        {/key}

        <!-- Interactive Action Buttons -->
        <div class="flex flex-wrap items-center gap-3 pt-2">
          <!-- Play Button -->
          <button
            onclick={playItem}
            class="relative inline-flex items-center justify-center gap-2 px-6 py-3 rounded-xl text-sm font-semibold text-slate-950 bg-gradient-to-r from-cyan-400 to-sky-400 hover:from-cyan-300 hover:to-sky-300 active:scale-95 transition-all shadow-[0_8px_20px_rgba(34,211,238,0.25)] hover:shadow-[0_12px_28px_rgba(34,211,238,0.4)] group/btn cursor-pointer"
          >
            <svg class="w-4 h-4 fill-current transition-transform group-hover/btn:scale-110" viewBox="0 0 24 24">
              <path d="M8 5v14l11-7z"/>
            </svg>
            Play Episode
          </button>

          <!-- More Info Button -->
          <button
            onclick={navigateToItem}
            class="px-5 py-3 rounded-xl text-sm font-semibold text-white border border-white/20 bg-white/5 hover:bg-white/10 hover:border-white/30 transition-all backdrop-blur-sm cursor-pointer"
          >
            Details
          </button>
        </div>

      </div>
    </div>

    <!-- Navigation dots -->
    {#if items.length > 1}
      <div class="absolute bottom-6 right-6 flex gap-2 z-20">
        {#each items as _, i}
          <button
            type="button"
            onclick={(e) => { e.stopPropagation(); goTo(i); }}
            class="w-2.5 h-2.5 rounded-full transition-all duration-300 cursor-pointer {i === currentIndex ? 'bg-cyan-400 w-6 shadow-[0_0_8px_rgba(34,211,238,0.6)]' : 'bg-white/30 hover:bg-white/50'}"
            aria-label="Go to slide {i + 1}"
          ></button>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .bg-radial-gradient-ambient {
    background: radial-gradient(circle at 10% 20%, rgba(6, 182, 212, 0.08) 0%, transparent 60%);
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes fadeInUp {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  :global(.animate-fade-in) {
    animation: fadeIn 0.6s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  :global(.animate-fade-in-up) {
    animation: fadeInUp 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }
</style>
