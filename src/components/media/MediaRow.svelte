<script lang="ts">
  import { onDestroy } from "svelte";
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

  let scrollerEl = $state<HTMLDivElement | null>(null);
  let isDragging = $state(false);
  let suppressClick = $state(false);
  let mouseStartX = 0;
  let scrollStartLeft = 0;
  let detachWindowListeners: (() => void) | null = null;

  function clearWindowListeners() {
    if (!detachWindowListeners) return;
    detachWindowListeners();
    detachWindowListeners = null;
  }

  function stopDragging() {
    if (!isDragging) return;
    isDragging = false;
    document.body.classList.remove("carousel-dragging");
    clearWindowListeners();

    // Keep suppression active for the click event that follows a drag.
    if (suppressClick) {
      window.setTimeout(() => {
        suppressClick = false;
      }, 0);
    }
  }

  function handleMouseDown(event: MouseEvent) {
    if (!scrollerEl) return;
    if (event.button !== 0) return;
    if (scrollerEl.scrollWidth <= scrollerEl.clientWidth) return;

    isDragging = true;
    suppressClick = false;
    mouseStartX = event.clientX;
    scrollStartLeft = scrollerEl.scrollLeft;
    document.body.classList.add("carousel-dragging");

    const handleMouseMove = (moveEvent: MouseEvent) => {
      if (!scrollerEl || !isDragging) return;

      const deltaX = moveEvent.clientX - mouseStartX;
      if (Math.abs(deltaX) > 4) {
        suppressClick = true;
      }

      if (suppressClick) {
        scrollerEl.scrollLeft = scrollStartLeft - deltaX;
        moveEvent.preventDefault();
      }
    };

    const handleMouseUp = () => {
      stopDragging();
    };

    window.addEventListener("mousemove", handleMouseMove, { passive: false });
    window.addEventListener("mouseup", handleMouseUp);
    window.addEventListener("blur", handleMouseUp);

    detachWindowListeners = () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      window.removeEventListener("blur", handleMouseUp);
    };

  }

  function handleMouseLeave() {
    if (!isDragging) return;
    stopDragging();
  }

  function handleClickCapture(event: MouseEvent) {
    if (!suppressClick) return;
    event.preventDefault();
    event.stopPropagation();
  }

  function handleScrollerKeyDown(event: KeyboardEvent) {
    if (!scrollerEl) return;

    const pageStep = Math.max(Math.round(scrollerEl.clientWidth * 0.85), 180);
    switch (event.key) {
      case "ArrowRight":
        scrollerEl.scrollBy({ left: pageStep, behavior: "smooth" });
        event.preventDefault();
        break;
      case "ArrowLeft":
        scrollerEl.scrollBy({ left: -pageStep, behavior: "smooth" });
        event.preventDefault();
        break;
      case "Home":
        scrollerEl.scrollTo({ left: 0, behavior: "smooth" });
        event.preventDefault();
        break;
      case "End":
        scrollerEl.scrollTo({ left: scrollerEl.scrollWidth, behavior: "smooth" });
        event.preventDefault();
        break;
      default:
        break;
    }
  }

  onDestroy(() => {
    if (isDragging) {
      stopDragging();
    } else {
      clearWindowListeners();
    }
    document.body.classList.remove("carousel-dragging");
  });
</script>

{#if items.length > 0}
  <section class="mb-6" aria-label={title}>
    <h2 class="text-lg font-semibold text-white mb-3 px-6">{title}</h2>
    <div class="relative">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        bind:this={scrollerEl}
        onmousedown={handleMouseDown}
        onmouseleave={handleMouseLeave}
        onkeydown={handleScrollerKeyDown}
        onclickcapture={handleClickCapture}
        tabindex="0"
        role="group"
        aria-label={`${title} carousel`}
        class="flex gap-3 overflow-x-auto px-6 pb-4 scrollbar-hide select-none cursor-grab active:cursor-grabbing rounded-md focus-visible:outline focus-visible:outline-2 focus-visible:outline-blue-400"
      >
        {#each items as item (item.id)}
          <PosterCard {item} {landscape} />
        {/each}
      </div>
    </div>
  </section>
{/if}
