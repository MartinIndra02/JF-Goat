import {
  getLatestItems,
  getLatestMedia,
  getNextUp,
  getResumeItems,
  getUserViews,
  loadHomepageCache,
  saveHomepageCache,
} from "../api";
import { filterSuppressedNextUp } from "../homepageFreshness";
import { isOnline, markDegraded, markHealthy } from "./connectivity.svelte";
import { pushErrorToast } from "./toast.svelte";
import type { MediaItem, UserLibrary, HomepageCache } from "../types";

class HomeDataStore {
  resumeItems = $state<MediaItem[]>([]);
  nextUpItems = $state<MediaItem[]>([]);
  userLibraries = $state<UserLibrary[]>([]);
  libraryLatest = $state<Record<string, MediaItem[]>>({});
  featuredItems = $state<MediaItem[]>([]);
  loading = $state(true);
  hasCachedData = $state(false);
  refreshing = $state(false);
  lastDataRefreshAt = $state<number | null>(null);

  async initializeHome() {
    await this.loadCachedThenRefresh();
  }

  async loadCachedThenRefresh() {
    try {
      const cached = await loadHomepageCache();
      if (cached) {
        this.applyHomepageData(cached);
        this.hasCachedData = true;
        this.loading = false;
      }
    } catch (error) {
      pushErrorToast("api", error, "Cache load failed", "homepage-cache-load-failed");
    }

    await this.refreshFromServer();
  }

  applyHomepageData(data: HomepageCache) {
    this.resumeItems = data.resume_items;
    this.nextUpItems = filterSuppressedNextUp(data.next_up_items);
    this.userLibraries = data.user_libraries;
    this.libraryLatest = data.library_latest;
    this.featuredItems = data.featured_items;
    if (typeof data.cache_refreshed_at_epoch_ms === "number") {
      this.lastDataRefreshAt = data.cache_refreshed_at_epoch_ms;
    }
  }

  async refreshFromServer() {
    if (!isOnline()) {
      this.loading = false;
      return;
    }

    this.refreshing = true;
    try {
      const [resume, nextUp, views, featured] = await Promise.all([
        getResumeItems(20),
        getNextUp(20),
        getUserViews(),
        getLatestMedia(10),
      ]);

      const filteredNextUp = filterSuppressedNextUp(nextUp);

      this.resumeItems = resume;
      this.nextUpItems = filteredNextUp;
      this.userLibraries = views;
      this.featuredItems = featured;

      const latestMap: Record<string, MediaItem[]> = {};
      if (views.length > 0) {
        const latestPromises = views.map(async (library) => {
          const items = await getLatestItems(library.id, 16).catch(() => []);
          return { id: library.id, items };
        });

        const results = await Promise.all(latestPromises);
        for (const result of results) {
          latestMap[result.id] = result.items;
        }
        this.libraryLatest = latestMap;
      }

      const refreshedAt = Date.now();
      this.lastDataRefreshAt = refreshedAt;
      markHealthy();

      await saveHomepageCache({
        resume_items: resume,
        next_up_items: filteredNextUp,
        user_libraries: views,
        library_latest: latestMap,
        featured_items: featured,
        cache_refreshed_at_epoch_ms: refreshedAt,
      });

      this.prefetchDetailImages([...resume, ...filteredNextUp]);
    } catch (error) {
      markDegraded(error);
      pushErrorToast(
        "api",
        error,
        "Live refresh failed",
        "dashboard-refresh-failed",
      );
    } finally {
      this.loading = false;
      this.refreshing = false;
    }
  }

  prefetchDetailImages(items: MediaItem[]) {
    const seen = new Set<string>();

    for (const item of items) {
      if (item.image_tag && !seen.has(`poster-${item.id}`)) {
        seen.add(`poster-${item.id}`);
        const img = new Image();
        img.src = `http://jfimage.localhost/poster/${item.id}?tag=${item.image_tag}`;
      }

      if (item.backdrop_tag && !seen.has(`backdrop-${item.id}`)) {
        seen.add(`backdrop-${item.id}`);
        const img = new Image();
        img.src = `http://jfimage.localhost/backdrop/${item.id}?tag=${item.backdrop_tag}`;
      }

      if (item.series_id && !seen.has(`poster-${item.series_id}`)) {
        seen.add(`poster-${item.series_id}`);
        const img = new Image();
        img.src = `http://jfimage.localhost/poster/${item.series_id}?tag=${item.series_id}`;
      }
    }
  }
}

export const homeDataStore = new HomeDataStore();
