<script lang="ts">
  import type { MediaItem } from "../../lib/types";

  let {
    item,
    landscape = false,
  }: {
    item: MediaItem;
    landscape?: boolean;
  } = $props();

  function formatRuntime(ticks: number | null): string {
    if (!ticks) return "";
    const minutes = Math.round(ticks / 600_000_000);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    const mins = minutes % 60;
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  }

  function progressPercent(item: MediaItem): number {
    if (!item.run_time_ticks || item.playback_ticks <= 0) return 0;
    return Math.min((item.playback_ticks / item.run_time_ticks) * 100, 100);
  }

  const progress = $derived(progressPercent(item));
  const aspectClass = $derived(landscape ? "aspect-video" : "aspect-[2/3]");
</script>

<div class="group cursor-pointer flex-shrink-0 {landscape ? 'w-56 sm:w-64' : 'w-32 sm:w-36'}">
  <div class="relative overflow-hidden rounded-lg shadow-md transition-transform duration-200 group-hover:scale-105 group-hover:shadow-xl">
    {#if landscape && item.backdrop_tag}
      <img
        src={`http://jfimage.localhost/backdrop/${item.id}?tag=${item.backdrop_tag}`}
        alt={item.name}
        loading="lazy"
        class="w-full {aspectClass} object-cover"
      />
    {:else if item.image_tag}
      <img
        src={`http://jfimage.localhost/poster/${item.id}?tag=${item.image_tag}`}
        alt={item.name}
        loading="lazy"
        class="w-full {aspectClass} object-cover"
      />
    {:else}
      <div class="w-full {aspectClass} bg-gray-800 flex items-center justify-center">
        <span class="text-gray-400 text-xs text-center px-2 line-clamp-3">{item.name}</span>
      </div>
    {/if}

    <div class="absolute inset-0 bg-black/0 group-hover:bg-black/30 transition-colors duration-200"></div>

    {#if progress > 0}
      <div class="absolute bottom-0 left-0 right-0 h-1 bg-black/50">
        <div
          class="h-full bg-blue-500 rounded-r-full"
          style="width: {progress}%"
        ></div>
      </div>
    {/if}

    {#if item.type === "Episode" && item.series_name}
      <div class="absolute top-1.5 left-1.5">
        <span class="bg-black/70 text-white text-[10px] px-1.5 py-0.5 rounded-sm backdrop-blur-sm">
          S{item.index_number ?? "?"}
        </span>
      </div>
    {/if}
  </div>

  <div class="mt-1.5 px-0.5">
    <p class="text-sm text-gray-200 truncate font-medium">{item.name}</p>
    {#if item.type === "Episode" && item.series_name}
      <p class="text-xs text-gray-400 truncate">{item.series_name}</p>
    {:else}
      <div class="flex items-center gap-1.5 text-xs text-gray-500">
        {#if item.production_year}
          <span>{item.production_year}</span>
        {/if}
        {#if item.community_rating}
          <span class="flex items-center gap-0.5">
            <svg class="w-3 h-3 text-yellow-500" viewBox="0 0 20 20" fill="currentColor">
              <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"/>
            </svg>
            {item.community_rating.toFixed(1)}
          </span>
        {/if}
        {#if item.run_time_ticks}
          <span>{formatRuntime(item.run_time_ticks)}</span>
        {/if}
      </div>
    {/if}
  </div>
</div>
