<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  import { scrollCarousel } from "../detailHelpers";

  interface Props {
    title: string;
    items: T[];
    getKey?: (item: T, index: number) => string | number;
    headerExtra?: Snippet;
    renderCard: Snippet<[T]>;
  }

  let {
    title,
    items,
    getKey = (_, i) => i,
    headerExtra,
    renderCard,
  }: Props = $props();

  let scrollEl = $state<HTMLElement | null>(null);
</script>

<div class="mb-8">
  <div class="flex items-center justify-between mb-3">
    <h2 class="text-base font-semibold text-white">{title}</h2>
    <div class="flex items-center gap-3">
      {#if headerExtra}
        {@render headerExtra()}
      {/if}
      <div class="flex items-center gap-1">
        <button
          aria-label="Scroll left"
          onclick={() => scrollCarousel(scrollEl, 'left')}
          class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"
        >
          <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M12.707 5.293a1 1 0 010 1.414L9.414 10l3.293 3.293a1 1 0 01-1.414 1.414l-4-4a1 1 0 010-1.414l4-4a1 1 0 011.414 0z" clip-rule="evenodd"/>
          </svg>
        </button>
        <button
          aria-label="Scroll right"
          onclick={() => scrollCarousel(scrollEl, 'right')}
          class="p-1.5 rounded-full hover:bg-white/10 transition-colors text-gray-400"
        >
          <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M7.293 14.707a1 1 0 010-1.414L10.586 10 7.293 6.707a1 1 0 011.414-1.414l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414 0z" clip-rule="evenodd"/>
          </svg>
        </button>
      </div>
    </div>
  </div>

  <div bind:this={scrollEl} class="flex gap-3 overflow-x-auto pb-4 scrollbar-hide -mx-5 px-5">
    {#each items as item, i (getKey(item, i))}
      {@render renderCard(item)}
    {/each}
  </div>
</div>
