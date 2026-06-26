<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { push } from "svelte-spa-router";
  import {
    getOfflineDownloads,
    mpvPlay,
    pauseDownload,
    resumeDownload,
    cancelDownload,
    deleteDownload,
  } from "../../lib/api";
  import { showPlayer } from "../../lib/stores/player.svelte";
  import { pushErrorToast } from "../../lib/stores/toast.svelte";
  import Button from "../../components/ui/Button.svelte";
  import type { OfflineDownload } from "../../lib/types";
  import { seasonNumber } from "../detail/detailHelpers";

  let activeRouteHeading = $state<HTMLElement | null>(null);
  let offlineDownloads = $state<OfflineDownload[]>([]);
  let loadingOffline = $state(false);

  function formatBytes(bytes: number | null | undefined): string {
    if (bytes === null || bytes === undefined || bytes <= 0) return "";
    const mb = bytes / (1024 * 1024);
    if (mb < 1024) {
      return `${mb.toFixed(1)} MB`;
    }
    const gb = mb / 1024;
    return `${gb.toFixed(1)} GB`;
  }

  async function loadOfflineDownloads() {
    loadingOffline = true;
    try {
      offlineDownloads = await getOfflineDownloads();
    } catch (error) {
      console.error("Failed to load offline downloads:", error);
    } finally {
      loadingOffline = false;
    }
  }

  onMount(() => {
    void loadOfflineDownloads();

    void tick().then(() => {
      activeRouteHeading?.focus();
    });

    let unlistenProgress: (() => void) | null = null;

    const setupListener = async () => {
      unlistenProgress = await listen<OfflineDownload>("download-progress", (event) => {
        const payload = event.payload;
        
        const index = offlineDownloads.findIndex(d => d.id === payload.id);
        if (index !== -1) {
          if (payload.status === "Deleted" || payload.status === "Cancelled") {
            offlineDownloads = offlineDownloads.filter(d => d.id !== payload.id);
          } else {
            offlineDownloads[index] = { ...offlineDownloads[index], ...payload };
            offlineDownloads = [...offlineDownloads];
          }
        } else if (payload.status !== "Deleted" && payload.status !== "Cancelled") {
          offlineDownloads = [payload, ...offlineDownloads];
        }
      });
    };

    void setupListener();

    return () => {
      if (unlistenProgress) unlistenProgress();
    };
  });

  async function playDownloadedItem(download: OfflineDownload) {
    showPlayer(download.id, download.name);
    try {
      await mpvPlay({
        itemId: download.id,
        startTicks: 0,
      });
    } catch (error) {
      pushErrorToast("player", error, "Failed to start offline playback", "offline-play-failed");
    }
  }

  async function handlePauseDownload(download: OfflineDownload) {
    try {
      await pauseDownload(download.id);
    } catch (error) {
      pushErrorToast("api", error, "Failed to pause download", "offline-pause-failed");
    }
  }

  async function handleResumeDownload(download: OfflineDownload) {
    try {
      await resumeDownload(download.id);
    } catch (error) {
      pushErrorToast("api", error, "Failed to resume download", "offline-resume-failed");
    }
  }

  async function handleCancelDownload(download: OfflineDownload) {
    try {
      await cancelDownload(download.id);
    } catch (error) {
      pushErrorToast("api", error, "Failed to cancel download", "offline-cancel-failed");
    }
  }

  async function handleDeleteDownload(download: OfflineDownload) {
    if (!confirm(`Are you sure you want to delete "${download.name}" from your offline synced media?`)) {
      return;
    }
    try {
      await deleteDownload(download.id);
    } catch (error) {
      pushErrorToast("api", error, "Failed to delete download", "offline-delete-failed");
    }
  }
</script>

<section class="px-6 pt-6 pb-10 max-w-4xl app-animate-fade-up" aria-label="Offline synced media">
  <div class="flex items-center justify-between gap-3 mb-6">
    <h2 bind:this={activeRouteHeading} tabindex="-1" class="text-xl font-semibold focus:outline-none">Offline Synced Media</h2>
    <span class="text-xs app-badge px-2.5 py-1">
      {offlineDownloads.length} {offlineDownloads.length === 1 ? 'item' : 'items'} synced
    </span>
  </div>

  {#if loadingOffline && offlineDownloads.length === 0}
    <div class="flex items-center justify-center h-48">
      <p class="app-muted text-sm">Loading synced media...</p>
    </div>
  {:else if offlineDownloads.length === 0}
    <div class="glass-panel rounded-2xl p-8 text-center flex flex-col items-center justify-center min-h-[300px]">
      <svg
        class="w-16 h-16 text-gray-700 mb-4"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        aria-hidden="true"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5M16.5 12L12 16.5m0 0L7.5 12m4.5 4.5V3"
        />
      </svg>
      <h3 class="text-base font-semibold text-white mb-1">No Offline Media</h3>
      <p class="app-muted text-sm max-w-sm mb-4">
        You haven't downloaded any media yet. Go to any Movie or Episode details page to download it for offline viewing.
      </p>
      <Button variant="secondary" onclick={() => push("/home")}>
        <span class="text-sm">Browse Home</span>
      </Button>
    </div>
  {:else}
    <div class="space-y-3">
      {#each offlineDownloads as download (download.id)}
        <div class="glass-panel rounded-2xl p-4 flex flex-col sm:flex-row sm:items-center justify-between gap-4 hover:border-white/20 transition-colors">
          <div class="flex items-center gap-4 min-w-0">
            <div class="w-16 h-24 rounded-xl overflow-hidden bg-[rgba(8,13,24,0.84)] shrink-0 border border-white/10 relative">
              <img
                src={download.type === "Episode" && download.series_id && download.series_image_tag
                  ? `http://jfimage.localhost/poster/${download.series_id}?tag=${download.series_image_tag}`
                  : download.image_tag
                    ? `http://jfimage.localhost/poster/${download.id}?tag=${download.image_tag}`
                    : `http://jfimage.localhost/poster/${download.id}?tag=${download.id}`}
                alt={download.name}
                loading="lazy"
                class="w-full h-full object-cover"
                onerror={(e) => {
                  const target = e.target as HTMLImageElement;
                  target.style.display = "none";
                }}
              />
              <div class="absolute inset-0 flex items-center justify-center text-gray-500 text-[10px] bg-slate-950/20 text-center px-1 pointer-events-none">
                {#if download.type === "Episode"}
                  EP
                {:else}
                  MV
                {/if}
              </div>
            </div>

            <div class="min-w-0">
              <h3 class="text-sm font-semibold text-white truncate">
                {#if download.type === "Episode" && download.series_name}
                  {download.series_name} - {download.name}
                {:else}
                  {download.name}
                {/if}
              </h3>
              <p class="text-xs app-muted mt-1">
                {#if download.type === "Episode"}
                  Episode
                  {#if download.season_name || download.index_number !== null}
                    · S{seasonNumber(download.season_name)}{download.index_number !== null ? `E${download.index_number}` : ''}
                  {/if}
                {:else}
                  Movie
                {/if}
                {#if download.status === "Completed" && download.total_bytes > 0}
                  · {formatBytes(download.total_bytes)}
                {/if}
                {#if download.status === "Completed"}
                  · Synced successfully
                {/if}
              </p>

              {#if download.status === "Downloading"}
                <div class="mt-2 flex flex-col gap-1 w-64 max-w-full">
                  <div class="flex justify-between text-[11px] app-faint">
                    <span>Downloading ({download.progress.toFixed(0)}%{#if download.total_bytes > 0} · {formatBytes(download.downloaded_bytes)} of {formatBytes(download.total_bytes)}{/if})</span>
                    {#if download.speed_bytes_per_sec > 0}
                      <span>{(download.speed_bytes_per_sec / (1024 * 1024)).toFixed(1)} MB/s</span>
                    {/if}
                  </div>
                  <div class="h-1.5 w-full bg-white/10 rounded-full overflow-hidden">
                    <div class="h-full bg-cyan-400 rounded-full transition-all duration-300" style="width: {download.progress}%"></div>
                  </div>
                </div>
              {:else if download.status === "Pending"}
                <span class="mt-2 inline-flex items-center gap-1.5 text-xs text-amber-300 bg-amber-500/10 px-2 py-0.5 rounded-md border border-amber-500/20">
                  <svg class="w-3 h-3 animate-pulse" fill="currentColor" viewBox="0 0 20 20"><path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-12a1 1 0 10-2 0v4a1 1 0 00.293.707l2.828 2.829a1 1 0 101.415-1.415L11 9.586V6z" clip-rule="evenodd"/></svg>
                  Queued
                </span>
              {:else if download.status === "Paused"}
                <span class="mt-2 inline-flex items-center gap-1.5 text-xs text-gray-300 bg-white/8 px-2 py-0.5 rounded-md">
                  Paused ({download.progress.toFixed(0)}%{#if download.total_bytes > 0} · {formatBytes(download.downloaded_bytes)} of {formatBytes(download.total_bytes)}{/if})
                </span>
              {:else if download.status === "Failed"}
                <div class="mt-2 text-xs text-red-400 flex flex-col gap-0.5">
                  <span class="font-semibold">Failed to download</span>
                  {#if download.error_message}
                    <span class="app-faint text-[10px] truncate max-w-xs">{download.error_message}</span>
                  {/if}
                </div>
              {/if}
            </div>
          </div>

          <div class="flex items-center gap-2 self-end sm:self-center shrink-0">
            {#if download.status === "Completed"}
              <Button variant="primary" onclick={() => playDownloadedItem(download)}>
                <div class="flex items-center gap-1.5">
                  <svg class="w-4 h-4" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
                  <span class="text-sm font-semibold">Play</span>
                </div>
              </Button>
              <button
                onclick={() => handleDeleteDownload(download)}
                class="h-10 w-10 grid place-items-center rounded-xl bg-white/5 hover:bg-red-500/15 border border-white/10 hover:border-red-500/30 text-gray-400 hover:text-red-400 transition-all cursor-pointer"
                aria-label="Delete download"
              >
                <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M9 2a1 1 0 00-.894.553L7.382 4H4a1 1 0 000 2v10a2 2 0 002 2h8a2 2 0 002-2V6a1 1 0 100-2h-3.382l-.724-1.447A1 1 0 0011 2H9zM7 8a1 1 0 012 0v6a1 1 0 11-2 0V8zm5-1a1 1 0 00-1 1v6a1 1 0 102 0V8a1 1 0 00-1-1z" clip-rule="evenodd"/></svg>
              </button>
            {:else if download.status === "Downloading"}
              <Button variant="secondary" onclick={() => handlePauseDownload(download)}>
                <span class="text-xs">Pause</span>
              </Button>
              <Button variant="secondary" onclick={() => handleCancelDownload(download)}>
                <span class="text-xs">Cancel</span>
              </Button>
            {:else if download.status === "Paused"}
              <Button variant="secondary" onclick={() => handleResumeDownload(download)}>
                <span class="text-xs">Resume</span>
              </Button>
              <Button variant="secondary" onclick={() => handleCancelDownload(download)}>
                <span class="text-xs">Cancel</span>
              </Button>
            {:else if download.status === "Pending"}
              <Button variant="secondary" onclick={() => handleCancelDownload(download)}>
                <span class="text-xs">Cancel</span>
              </Button>
            {:else if download.status === "Failed"}
              <Button variant="secondary" onclick={() => handleResumeDownload(download)}>
                <span class="text-xs">Retry</span>
              </Button>
              <Button variant="secondary" onclick={() => handleCancelDownload(download)}>
                <span class="text-xs">Cancel</span>
              </Button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</section>
