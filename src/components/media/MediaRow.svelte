<script lang="ts">
  import type { MediaItem } from "../../lib/types";
  import PosterCard from "./PosterCard.svelte";

  let {
    title,
    items,
    landscape = false,
  }: {
    title: string;
    items: MediaItem[];
    landscape?: boolean;
  } = $props();
</script>

{#if items.length > 0}
  <section class="mb-6">
    <h2 class="text-lg font-semibold text-white mb-3 px-6">{title}</h2>
    <div class="relative">
      <div
        class="flex gap-3 overflow-x-auto px-6 pb-4 scrollbar-hide"
        style="scrollbar-width: none; -ms-overflow-style: none;"
      >
        {#each items as item (item.id)}
          <PosterCard {item} {landscape} />
        {/each}
      </div>
    </div>
  </section>
{/if}
