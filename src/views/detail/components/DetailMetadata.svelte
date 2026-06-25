<script lang="ts">
  import type { MediaItem } from "../../../lib/types";
  import { formatRuntime, formatDate } from "../detailHelpers";

  interface Props {
    item: MediaItem;
  }

  let { item }: Props = $props();
</script>

<!-- Metadata row -->
<div class="flex flex-wrap items-center justify-center gap-2 mb-3">
  {#if item.official_rating}
    <span class="inline-flex items-center gap-1 text-xs font-semibold text-gray-200 bg-white/10 px-2.5 py-1 rounded-md border border-white/15">
      {item.official_rating}
    </span>
    <span class="w-1 h-1 rounded-full bg-gray-500"></span>
  {/if}

  {#if item.date_created && item.type === "Episode"}
    <span class="inline-flex items-center gap-1.5 text-xs text-gray-300 bg-white/8 px-2.5 py-1 rounded-md">
      <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clip-rule="evenodd"/>
      </svg>
      {formatDate(item.date_created)}
    </span>
  {:else if item.production_year}
    <span class="inline-flex items-center gap-1.5 text-xs text-gray-300 bg-white/8 px-2.5 py-1 rounded-md">
      <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M6 2a1 1 0 00-1 1v1H4a2 2 0 00-2 2v10a2 2 0 002 2h12a2 2 0 002-2V6a2 2 0 00-2-2h-1V3a1 1 0 10-2 0v1H7V3a1 1 0 00-1-1zm0 5a1 1 0 000 2h8a1 1 0 100-2H6z" clip-rule="evenodd"/>
      </svg>
      {item.production_year}
    </span>
  {/if}

  {#if item.run_time_ticks}
    <span class="w-1 h-1 rounded-full bg-gray-500"></span>
    <span class="inline-flex items-center gap-1.5 text-xs text-gray-300 bg-white/8 px-2.5 py-1 rounded-md">
      <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd"/>
      </svg>
      {formatRuntime(item.run_time_ticks)}
    </span>
  {/if}

  {#if item.community_rating}
    <span class="w-1 h-1 rounded-full bg-gray-500"></span>
    <span class="inline-flex items-center gap-1.5 text-xs font-medium bg-amber-500/20 text-amber-300 px-2.5 py-1 rounded-md">
      <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
        <path d="M9.049 2.927c.3-.921 1.603-.921 1.902 0l1.07 3.292a1 1 0 00.95.69h3.462c.969 0 1.371 1.24.588 1.81l-2.8 2.034a1 1 0 00-.364 1.118l1.07 3.292c.3.921-.755 1.688-1.54 1.118l-2.8-2.034a1 1 0 00-1.175 0l-2.8 2.034c-.784.57-1.838-.197-1.539-1.118l1.07-3.292a1 1 0 00-.364-1.118L2.98 8.72c-.783-.57-.38-1.81.588-1.81h3.461a1 1 0 00.951-.69l1.07-3.292z"/>
      </svg>
      {item.community_rating.toFixed(1)}
    </span>
  {/if}
</div>

<!-- Genre badges -->
{#if item.genres}
  <div class="flex flex-wrap justify-center gap-1.5 mb-4">
    {#each item.genres.split(",").slice(0, 6) as genre}
      <span class="text-xs text-gray-300 bg-white/10 px-2.5 py-1 rounded-full border border-white/5">
        {genre.trim()}
      </span>
    {/each}
  </div>
{/if}
