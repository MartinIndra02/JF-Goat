<script lang="ts">
  import type { Snippet } from "svelte";
  import type { MediaItem } from "../../../lib/types";
  import { backdropUrl, handleImageLoad } from "../detailHelpers";
  import { push } from "svelte-spa-router";

  interface Props {
    item: MediaItem;
    goBack?: () => void;
    clampHeight?: string;
    rightControls?: Snippet;
  }

  let {
    item,
    goBack = () => {
      if (window.history.length > 1) {
        window.history.back();
      } else {
        push("/home");
      }
    },
    clampHeight = "clamp(340px, 55vh, 560px)",
    rightControls,
  }: Props = $props();
</script>

<div class="relative w-full overflow-hidden" style="height: {clampHeight};">
  {#if backdropUrl(item)}
    <img
      src={backdropUrl(item)}
      alt=""
      onload={handleImageLoad}
      class="absolute inset-0 w-full h-full object-cover object-top transition-opacity duration-500 opacity-0"
    />
  {/if}
  <div class="absolute inset-0 bg-[rgba(5,7,13,0.5)] -z-10"></div>
  <div class="absolute inset-0 bg-gradient-to-t from-[var(--bg-0)] via-[rgba(5,7,13,0.4)] to-transparent"></div>

  <!-- Back button -->
  <button
    aria-label="Go back"
    onclick={goBack}
    class="absolute top-4 left-4 z-10 h-10 w-10 grid place-items-center bg-[rgba(10,18,31,0.64)] border border-white/22 rounded-xl backdrop-blur-xl text-[var(--text-primary)] hover:bg-[rgba(22,34,54,0.76)] transition-colors"
  >
    <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path stroke-linecap="round" stroke-linejoin="round" d="M15 19l-7-7 7-7"/>
    </svg>
  </button>

  {#if rightControls}
    <div class="absolute top-4 right-4 z-10 flex items-center gap-1.5">
      {@render rightControls()}
    </div>
  {/if}
</div>
