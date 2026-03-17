import type { MediaItem } from "../../lib/types";

export const IMAGE_BASE = "http://jfimage.localhost";

export function seasonNumber(seasonName: string | null | undefined): string {
  return seasonName?.replace("Season ", "") ?? "?";
}

export function formatRuntime(ticks: number | null): string {
  if (!ticks) return "";
  const minutes = Math.round(ticks / 600_000_000);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
}

export function formatDate(dateStr: string | null): string {
  if (!dateStr) return "";
  try {
    const d = new Date(dateStr);
    return d.toLocaleDateString("en-US", {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return "";
  }
}

export function progressPercent(item: MediaItem): number {
  if (!item.run_time_ticks || !item.playback_ticks || item.playback_ticks <= 0)
    return 0;
  return Math.min((item.playback_ticks / item.run_time_ticks) * 100, 100);
}

export function handleImageLoad(event: Event) {
  const img = event.target as HTMLImageElement;
  if (img.naturalWidth <= 1 && img.naturalHeight <= 1) {
    const src = img.src;
    const retryCount = parseInt(img.dataset.retry ?? "0");
    if (retryCount < 3) {
      setTimeout(
        () => {
          img.dataset.retry = String(retryCount + 1);
          img.src = "";
          img.src = src;
        },
        1500 * (retryCount + 1),
      );
    }
  } else {
    img.classList.add("opacity-100");
  }
}

export function backdropUrl(itm: MediaItem): string {
  if (itm.backdrop_tag) {
    return `${IMAGE_BASE}/backdrop/${itm.id}?tag=${itm.backdrop_tag}`;
  }
  if (itm.series_id) {
    return `${IMAGE_BASE}/backdrop/${itm.series_id}?tag=${itm.series_id}`;
  }
  return "";
}

/** Episode thumbnail: prefer backdrop, then primary image, then series backdrop */
export function episodeThumbnailUrl(itm: MediaItem): string {
  if (itm.backdrop_tag) {
    return `${IMAGE_BASE}/backdrop/${itm.id}?tag=${itm.backdrop_tag}`;
  }
  if (itm.image_tag) {
    return `${IMAGE_BASE}/poster/${itm.id}?tag=${itm.image_tag}`;
  }
  if (itm.series_id) {
    return `${IMAGE_BASE}/backdrop/${itm.series_id}?tag=${itm.series_id}`;
  }
  return "";
}

export function posterUrl(itm: MediaItem): string {
  if (itm.image_tag) {
    return `${IMAGE_BASE}/poster/${itm.id}?tag=${itm.image_tag}`;
  }
  if (itm.series_id) {
    return `${IMAGE_BASE}/poster/${itm.series_id}?tag=${itm.series_id}`;
  }
  return "";
}

export function personImageUrl(id: string, imageTag: string | null): string {
  if (imageTag) {
    return `${IMAGE_BASE}/poster/${id}?tag=${imageTag}`;
  }
  return "";
}

/** Scroll a carousel container left or right by a page width */
export function scrollCarousel(
  container: HTMLElement | null,
  direction: "left" | "right",
) {
  if (!container) return;
  const scrollAmount = container.clientWidth * 0.8;
  container.scrollBy({
    left: direction === "left" ? -scrollAmount : scrollAmount,
    behavior: "smooth",
  });
}
