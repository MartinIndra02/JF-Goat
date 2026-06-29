<script lang="ts">
  import { onMount, tick } from "svelte";
  import { push } from "svelte-spa-router";
  import { getVersion } from "@tauri-apps/api/app";
  import {
    getUpdateStatus,
    getUpdateInfo,
    getDownloadProgress,
    getUpdateError,
    checkForUpdates,
    installUpdate,
    performRestart,
    dismissUpdate,
  } from "../../lib/stores/updater.svelte";
  import {
    isPreferencesLoaded,
    isPreferencesSaving,
    getPreferences,
    loadPreferences,
    updatePreferences,
  } from "../../lib/stores/preferences.svelte";
  import { isOnline } from "../../lib/stores/connectivity.svelte";
  import { pushToast, pushErrorToast } from "../../lib/stores/toast.svelte";
  import {
    logout as logoutApi,
    selectDownloadDirectory,
    exportDiagnostics,
    forceResync,
    getStorageUsage,
    clearAppCache,
    deleteAllOfflineDownloads,
  } from "../../lib/api";
  import { resetSyncStore } from "../../lib/stores/sync.svelte";
  import { setUnauthenticated } from "../../lib/stores/auth.svelte";
  import Button from "../../components/ui/Button.svelte";

  import { homeDataStore } from "../../lib/stores/homeData.svelte";

  let activeRouteHeading = $state<HTMLElement | null>(null);

  const preferences = $derived(getPreferences());
  const preferencesLoaded = $derived(isPreferencesLoaded());
  const preferencesSaving = $derived(isPreferencesSaving());

  const online = $derived(isOnline());

  const updaterStatus = $derived(getUpdateStatus());
  const updaterInfo = $derived(getUpdateInfo());
  const updaterProgress = $derived(getDownloadProgress());
  const updaterError = $derived(getUpdateError());

  import type { StorageUsage } from "../../lib/types";

  let appVersion = $state("...");
  let downloadingDiagnostics = $state(false);
  let runningResync = $state(false);
  let activeTab = $state("general");

  let storageUsage = $state<StorageUsage | null>(null);
  let loadingStorage = $state(false);
  let clearingCache = $state(false);
  let deletingDownloads = $state(false);

  const SUBTITLE_POSITION_STORAGE_KEY = "jfgoat.player.subtitleBottomPercent";
  const DEFAULT_SUBTITLE_POSITION_PERCENT = 92;
  let subtitlePositionPercent = $state(readStoredSubtitlePositionPercent());

  onMount(() => {
    subtitlePositionPercent = readStoredSubtitlePositionPercent();
    
    getVersion()
      .then((v) => (appVersion = v))
      .catch(() => (appVersion = "unknown"));

    if (!preferencesLoaded) {
      void loadPreferences();
    }

    void loadStorageUsage();

    void tick().then(() => {
      activeRouteHeading?.focus();
    });
  });

  async function loadStorageUsage() {
    loadingStorage = true;
    try {
      storageUsage = await getStorageUsage();
    } catch (e) {
      console.error("Failed to load storage metrics:", e);
    } finally {
      loadingStorage = false;
    }
  }

  async function handleClearCache() {
    if (clearingCache) return;
    clearingCache = true;
    try {
      await clearAppCache();
      pushToast({
        level: "success",
        source: "system",
        title: "Cache cleared",
        message: "Application cache size has been reset.",
        dismissAfterMs: 3000,
      });
      await loadStorageUsage();
    } catch (error) {
      pushErrorToast("system", error, "Failed to clear cache", "clear-cache-failed");
    } finally {
      clearingCache = false;
    }
  }

  async function handleDeleteAllDownloads() {
    if (deletingDownloads) return;
    deletingDownloads = true;
    try {
      await deleteAllOfflineDownloads();
      pushToast({
        level: "success",
        source: "system",
        title: "Downloads deleted",
        message: "All offline downloaded movies and episodes have been deleted.",
        dismissAfterMs: 3000,
      });
      await loadStorageUsage();
    } catch (error) {
      pushErrorToast("system", error, "Failed to delete downloads", "delete-downloads-failed");
    } finally {
      deletingDownloads = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 Bytes";
    const k = 1024;
    const sizes = ["Bytes", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function setHwdecPreference(event: Event) {
    const target = event.target as HTMLSelectElement;
    updatePreferences({
      playback: {
        hwdec: target.value,
      },
    });
  }

  function setSkipForwardPreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      playback: {
        skip_forward_seconds: Number(target.value),
      },
    });
  }

  function setSkipBackwardPreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      playback: {
        skip_backward_seconds: Number(target.value),
      },
    });
  }

  function setSubtitleSizePreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      playback: {
        subtitle_size_percent: Number(target.value),
      },
    });
  }

  function setSubtitleColorPreference(event: Event) {
    const target = event.target as HTMLSelectElement;
    updatePreferences({
      playback: {
        subtitle_color: target.value,
      },
    });
  }

  function setSubtitleBackgroundOpacityPreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      playback: {
        subtitle_background_opacity: Number(target.value),
      },
    });
  }

  function setDefaultStartupScreenPreference(event: Event) {
    const target = event.target as HTMLSelectElement;
    updatePreferences({
      playback: {
        default_startup_screen: target.value,
      },
    });
  }

  function setAutoplayPreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      playback: {
        autoplay_next_episode: target.checked,
      },
    });
  }

  function setPlaybackRatePreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      playback: {
        default_playback_rate: Number(target.value),
      },
    });
  }

  function setAudioLanguagePreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      language: {
        preferred_audio_language: target.value,
      },
    });
  }

  function setSubtitleLanguagePreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      language: {
        preferred_subtitle_language: target.value,
      },
    });
  }

  // Quality preference local storage sync handles internally on changes
  $effect(() => {
    if (!preferencesLoaded) return;
    if (typeof localStorage === "undefined") return;

    localStorage.setItem(
      "jfgoat.player.defaultPlaybackRate",
      String(preferences.playback.default_playback_rate),
    );
    localStorage.setItem(
      "jfgoat.player.defaultQualityKey",
      preferences.quality.default_quality_key,
    );

    if (preferences.language.preferred_audio_language) {
      localStorage.setItem(
        "jfgoat.player.preferredAudioLanguage",
        preferences.language.preferred_audio_language,
      );
    }

    if (preferences.language.preferred_subtitle_language) {
      localStorage.setItem(
        "jfgoat.player.preferredSubtitleLanguage",
        preferences.language.preferred_subtitle_language,
      );
    }
  });

  function setQualityPreference(event: Event) {
    const target = event.target as HTMLSelectElement;
    updatePreferences({
      quality: {
        default_quality_key: target.value,
      },
    });
  }

  function setCacheEnabledPreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      cache: {
        enabled: target.checked,
      },
    });
  }

  function setCacheAgePreference(event: Event) {
    const target = event.target as HTMLInputElement;
    updatePreferences({
      cache: {
        max_age_minutes: Number(target.value),
      },
    });
  }

  function setRefreshIntervalPreference(event: Event) {
    const target = event.target as HTMLSelectElement;
    updatePreferences({
      refresh_interval_seconds: Number(target.value),
    });
  }

  async function changeDownloadDirectory() {
    try {
      const selected = await selectDownloadDirectory();
      if (selected) {
        updatePreferences({
          download_directory: selected,
        });
        pushToast({
          level: "success",
          source: "api",
          title: "Download location updated",
          message: `Saved to ${selected}`,
          dismissAfterMs: 3000,
        });
      }
    } catch (error) {
      pushErrorToast("api", error, "Failed to change download location", "settings-download-dir-failed");
    }
  }

  function resetDownloadDirectory() {
    updatePreferences({
      download_directory: null,
    });
    pushToast({
      level: "info",
      source: "api",
      title: "Download location reset",
      message: "Using default application downloads directory.",
      dismissAfterMs: 3000,
    });
  }

  function clampSubtitlePositionPercent(value: number): number {
    return Math.max(70, Math.min(98, Math.round(value)));
  }

  function readStoredSubtitlePositionPercent(): number {
    if (typeof localStorage === "undefined") {
      return DEFAULT_SUBTITLE_POSITION_PERCENT;
    }

    const raw = localStorage.getItem(SUBTITLE_POSITION_STORAGE_KEY);
    if (!raw) {
      return DEFAULT_SUBTITLE_POSITION_PERCENT;
    }

    const parsed = Number(raw);
    if (!Number.isFinite(parsed)) {
      return DEFAULT_SUBTITLE_POSITION_PERCENT;
    }

    return clampSubtitlePositionPercent(parsed);
  }

  function setSubtitlePositionPreference(event: Event) {
    const target = event.target as HTMLInputElement;
    subtitlePositionPercent = clampSubtitlePositionPercent(Number(target.value));

    if (typeof localStorage !== "undefined") {
      localStorage.setItem(
        SUBTITLE_POSITION_STORAGE_KEY,
        String(subtitlePositionPercent),
      );
    }
  }

  async function handleDownloadDiagnostics() {
    if (downloadingDiagnostics) return;

    downloadingDiagnostics = true;
    try {
      const result = await exportDiagnostics();
      pushToast({
        level: "success",
        source: "system",
        title: "Diagnostics exported",
        message: `Saved to ${result.file_path}`,
        dedupeKey: `diagnostics-export-${result.generated_at_unix_ms}`,
      });
    } catch (error) {
      pushErrorToast(
        "system",
        error,
        "Diagnostics export failed",
        "diagnostics-export-failed",
      );
    } finally {
      downloadingDiagnostics = false;
    }
  }

  async function handleResync() {
    if (runningResync) return;
    if (!online) {
      pushToast({
        level: "warning",
        source: "sync",
        title: "Offline",
        message: "Reconnect before forcing a resync.",
        dedupeKey: "resync-offline",
      });
      return;
    }

    runningResync = true;
    try {
      await forceResync();
      await homeDataStore.refreshFromServer();
      pushToast({
        level: "success",
        source: "sync",
        title: "Resync started",
        message: "Your library is syncing in the background.",
        dedupeKey: "resync-started",
      });
    } catch (error) {
      pushErrorToast("sync", error, "Resync failed", "force-resync-failed");
    } finally {
      runningResync = false;
    }
  }

  async function handleLogout() {
    try {
      await logoutApi();
    } catch {
      // Best effort only.
    }

    resetSyncStore();
    setUnauthenticated();
    push("/connect");
  }
</script>

<section class="px-6 pt-6 pb-10 max-w-4xl app-animate-fade-up" aria-label="Settings">
  <div class="flex flex-wrap items-center justify-between gap-3 mb-6">
    <h2 bind:this={activeRouteHeading} tabindex="-1" class="text-xl font-semibold focus:outline-none">Preferences</h2>
    {#if preferencesSaving}
      <span class="text-xs app-badge px-2 py-1">Saving...</span>
    {/if}
  </div>

  <div class="flex flex-col md:flex-row gap-6">
    <!-- Tabs Navigation -->
    <aside class="flex flex-row md:flex-col gap-1 overflow-x-auto scrollbar-hide border-b md:border-b-0 md:border-r border-white/10 pb-3 md:pb-0 md:pr-6 shrink-0 md:w-60">
      <button
        type="button"
        onclick={() => activeTab = "general"}
        class="flex items-center gap-3 px-4 py-2.5 rounded-xl text-sm font-medium tracking-[0.01em] transition-all duration-200 shrink-0 border {activeTab === 'general' ? 'bg-cyan-500/10 border-cyan-400/20 text-cyan-300' : 'border-transparent text-[var(--text-secondary)] hover:text-white hover:bg-white/5'}"
      >
        <svg class="w-4 h-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3"/>
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
        General
      </button>
      <button
        type="button"
        onclick={() => activeTab = "player"}
        class="flex items-center gap-3 px-4 py-2.5 rounded-xl text-sm font-medium tracking-[0.01em] transition-all duration-200 shrink-0 border {activeTab === 'player' ? 'bg-cyan-500/10 border-cyan-400/20 text-cyan-300' : 'border-transparent text-[var(--text-secondary)] hover:text-white hover:bg-white/5'}"
      >
        <svg class="w-4 h-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <polygon points="6 3 20 12 6 21 6 3"/>
        </svg>
        Player & Playback
      </button>
      <button
        type="button"
        onclick={() => activeTab = "storage"}
        class="flex items-center gap-3 px-4 py-2.5 rounded-xl text-sm font-medium tracking-[0.01em] transition-all duration-200 shrink-0 border {activeTab === 'storage' ? 'bg-cyan-500/10 border-cyan-400/20 text-cyan-300' : 'border-transparent text-[var(--text-secondary)] hover:text-white hover:bg-white/5'}"
      >
        <svg class="w-4 h-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <rect x="2" y="2" width="20" height="8" rx="2" ry="2"/>
          <rect x="2" y="14" width="20" height="8" rx="2" ry="2"/>
          <line x1="6" y1="6" x2="6.01" y2="6"/>
          <line x1="6" y1="18" x2="6.01" y2="18"/>
        </svg>
        Downloads & Storage
      </button>
      <button
        type="button"
        onclick={() => activeTab = "maintenance"}
        class="flex items-center gap-3 px-4 py-2.5 rounded-xl text-sm font-medium tracking-[0.01em] transition-all duration-200 shrink-0 border {activeTab === 'maintenance' ? 'bg-cyan-500/10 border-cyan-400/20 text-cyan-300' : 'border-transparent text-[var(--text-secondary)] hover:text-white hover:bg-white/5'}"
      >
        <svg class="w-4 h-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>
        </svg>
        System & Updates
      </button>
      <button
        type="button"
        onclick={() => activeTab = "about"}
        class="flex items-center gap-3 px-4 py-2.5 rounded-xl text-sm font-medium tracking-[0.01em] transition-all duration-200 shrink-0 border {activeTab === 'about' ? 'bg-cyan-500/10 border-cyan-400/20 text-cyan-300' : 'border-transparent text-[var(--text-secondary)] hover:text-white hover:bg-white/5'}"
      >
        <svg class="w-4 h-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"/>
          <line x1="12" y1="16" x2="12" y2="12"/>
          <line x1="12" y1="8" x2="12.01" y2="8"/>
        </svg>
        About
      </button>
    </aside>

    <!-- Tab Content -->
    <div class="flex-1 min-w-0 space-y-4">
      {#if activeTab === "general"}
        <div class="glass-panel rounded-2xl p-4">
          <h3 class="text-sm font-semibold mb-3">Application Behavior</h3>
          <div class="grid gap-3 md:grid-cols-2">
            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1 md:col-span-2">
              Default Startup View
              <select
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
                value={preferences.playback.default_startup_screen}
                onchange={setDefaultStartupScreenPreference}
              >
                <option value="/home" class="bg-gray-900">Home Dashboard</option>
                <option value="/library" class="bg-gray-900">Library View</option>
                <option value="/offline" class="bg-gray-900">Offline Downloads</option>
              </select>
            </label>
          </div>
        </div>

        <div class="glass-panel rounded-2xl p-4">
          <h3 class="text-sm font-semibold mb-3">Preferred Languages</h3>
          <div class="grid gap-3 md:grid-cols-2">
            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
              Preferred audio language
              <input
                type="text"
                value={preferences.language.preferred_audio_language}
                onchange={setAudioLanguagePreference}
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
                placeholder="en, cs, de..."
              />
            </label>

            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
              Preferred subtitle language
              <input
                type="text"
                value={preferences.language.preferred_subtitle_language}
                onchange={setSubtitleLanguagePreference}
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
                placeholder="en, cs, de..."
              />
            </label>
          </div>
        </div>

        <div class="glass-panel rounded-2xl p-4">
          <h3 class="text-sm font-semibold mb-3">Library Updates</h3>
          <div class="grid gap-3 md:grid-cols-2">
            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1 md:col-span-2">
              Metadata refresh interval
              <select
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
                value={String(preferences.refresh_interval_seconds)}
                onchange={setRefreshIntervalPreference}
              >
                <option value="30" class="bg-gray-900">Every 30s</option>
                <option value="60" class="bg-gray-900">Every 1 min</option>
                <option value="180" class="bg-gray-900">Every 3 min</option>
                <option value="300" class="bg-gray-900">Every 5 min</option>
                <option value="600" class="bg-gray-900">Every 10 min</option>
              </select>
            </label>
          </div>
        </div>

      {:else if activeTab === "player"}
        <div class="glass-panel rounded-2xl p-4">
          <h3 class="text-sm font-semibold mb-3">Video Playback</h3>
          <div class="grid gap-4 md:grid-cols-2">
            <label class="text-sm text-[var(--text-secondary)] flex items-center gap-2 md:col-span-2">
              <input
                type="checkbox"
                checked={preferences.playback.autoplay_next_episode}
                onchange={setAutoplayPreference}
              />
              Autoplay next episode
            </label>

            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
              Default playback speed
              <input
                type="number"
                min="0.5"
                max="2"
                step="0.1"
                value={preferences.playback.default_playback_rate}
                onchange={setPlaybackRatePreference}
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
              />
            </label>

            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
              Hardware Decoding
              <select
                value={preferences.playback.hwdec}
                onchange={setHwdecPreference}
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
              >
                <option value="auto" class="bg-gray-900">Auto-detect (Recommended)</option>
                <option value="no" class="bg-gray-900">Disabled (Software only)</option>
                <option value="d3d11va" class="bg-gray-900">Direct3D 11 (Windows)</option>
                <option value="dxva2" class="bg-gray-900">DXVA2 (Windows Legacy)</option>
                <option value="nvdec" class="bg-gray-900">Nvidia NVDEC</option>
              </select>
            </label>

            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1 md:col-span-2">
              Default streaming quality
              <select
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
                value={preferences.quality.default_quality_key}
                onchange={setQualityPreference}
              >
                <option value="direct-play" class="bg-gray-900">Direct Play</option>
                <option value="preset-1080" class="bg-gray-900">1080p</option>
                <option value="preset-720" class="bg-gray-900">720p</option>
                <option value="preset-480" class="bg-gray-900">480p</option>
              </select>
            </label>
          </div>
        </div>

        <div class="glass-panel rounded-2xl p-4">
          <h3 class="text-sm font-semibold mb-3">Navigation Controls</h3>
          <div class="grid gap-4 md:grid-cols-2">
            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
              Seek Forward Skip (seconds)
              <input
                type="number"
                min="5"
                max="300"
                step="5"
                value={preferences.playback.skip_forward_seconds}
                onchange={setSkipForwardPreference}
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
              />
            </label>

            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
              Seek Backward Skip (seconds)
              <input
                type="number"
                min="5"
                max="300"
                step="5"
                value={preferences.playback.skip_backward_seconds}
                onchange={setSkipBackwardPreference}
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
              />
            </label>
          </div>
        </div>

        <div class="glass-panel rounded-2xl p-4">
          <h3 class="text-sm font-semibold mb-3">Subtitle Styling</h3>
          <div class="grid gap-4 md:grid-cols-2">
            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1 md:col-span-2">
              Subtitle vertical position
              <input
                type="range"
                min="70"
                max="98"
                step="1"
                value={subtitlePositionPercent}
                oninput={setSubtitlePositionPreference}
                class="accent-[#66d8ff]"
                aria-label="Subtitle vertical position"
              />
              <span class="text-xs app-faint">{subtitlePositionPercent}% from bottom</span>
            </label>

            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
              Subtitle Font Size Scale
              <input
                type="range"
                min="50"
                max="300"
                step="10"
                value={preferences.playback.subtitle_size_percent}
                oninput={setSubtitleSizePreference}
                class="accent-[#66d8ff]"
                aria-label="Subtitle font size scale"
              />
              <span class="text-xs app-faint">{preferences.playback.subtitle_size_percent}% size</span>
            </label>

            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
              Subtitle Background Opacity
              <input
                type="range"
                min="0"
                max="100"
                step="5"
                value={preferences.playback.subtitle_background_opacity}
                oninput={setSubtitleBackgroundOpacityPreference}
                class="accent-[#66d8ff]"
                aria-label="Subtitle background opacity"
              />
              <span class="text-xs app-faint">{preferences.playback.subtitle_background_opacity}% opacity</span>
            </label>

            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1 md:col-span-2">
              Subtitle Font Color
              <select
                value={preferences.playback.subtitle_color}
                onchange={setSubtitleColorPreference}
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
              >
                <option value="#ffffff" class="bg-gray-900">White</option>
                <option value="#ffff00" class="bg-gray-900">Yellow</option>
                <option value="#00ffff" class="bg-gray-900">Cyan</option>
                <option value="#00ff00" class="bg-gray-900">Green</option>
                <option value="#ff00ff" class="bg-gray-900">Magenta</option>
              </select>
            </label>
          </div>
        </div>

      {:else if activeTab === "storage"}
        <div class="glass-panel rounded-2xl p-4">
          <h3 class="text-sm font-semibold mb-3">Offline Downloads</h3>
          <div class="space-y-3">
            <div class="text-sm text-[var(--text-secondary)] flex flex-col gap-1.5">
              Download Location
              <div class="flex gap-2">
                <input
                  type="text"
                  readonly
                  value={preferences.download_directory || "Default (App Data folder/downloads)"}
                  class="flex-1 h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
                />
                <Button variant="secondary" onclick={changeDownloadDirectory}>
                  <span class="text-sm">Change</span>
                </Button>
                {#if preferences.download_directory}
                  <Button variant="secondary" onclick={resetDownloadDirectory}>
                    <span class="text-sm">Reset</span>
                  </Button>
                {/if}
              </div>
              <span class="text-[11px] app-faint">Select where offline movies and episodes will be stored on disk.</span>
            </div>
          </div>
        </div>

        <div class="glass-panel rounded-2xl p-4">
          <h3 class="text-sm font-semibold mb-3">Homepage Cache</h3>
          <div class="grid gap-3 md:grid-cols-2">
            <label class="text-sm text-[var(--text-secondary)] flex items-center gap-2 md:col-span-2">
              <input
                type="checkbox"
                checked={preferences.cache.enabled}
                onchange={setCacheEnabledPreference}
              />
              Enable local homepage cache
            </label>

            <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
              Cache max age (minutes)
              <input
                type="number"
                min="5"
                max="10080"
                value={preferences.cache.max_age_minutes}
                onchange={setCacheAgePreference}
                class="h-10 rounded-xl border border-white/14 bg-[rgba(13,21,35,0.72)] px-3 text-sm text-[var(--text-primary)]"
                disabled={!preferences.cache.enabled}
              />
            </label>
          </div>
        </div>

        <!-- Storage Cleanup -->
        <div class="glass-panel rounded-2xl p-4">
          <h3 class="text-sm font-semibold mb-4 text-white">Storage Cleanup</h3>
          <div class="space-y-4">
            <!-- Cache -->
            <div class="flex flex-col sm:flex-row sm:items-start justify-between gap-4 pb-4 border-b border-white/10">
              <div class="space-y-1 max-w-md md:max-w-lg">
                <h4 class="text-xs font-semibold text-white">Application Cache Metrics</h4>
                <p class="text-xs text-[var(--text-secondary)] leading-relaxed">
                  Holds transient cached items like dashboard layouts and network image fallbacks.
                </p>
                <span class="text-xs app-pill px-2.5 py-0.5 inline-block font-mono">
                  {#if loadingStorage}
                    Calculating...
                  {:else if storageUsage}
                    {formatBytes(storageUsage.cache_bytes)}
                  {:else}
                    --
                  {/if}
                </span>
              </div>
              <Button variant="secondary" onclick={handleClearCache} disabled={clearingCache} className="shrink-0">
                <span class="text-xs font-semibold">{clearingCache ? "Clearing..." : "Clear Cache"}</span>
              </Button>
            </div>

            <!-- Downloads -->
            <div class="flex flex-col sm:flex-row sm:items-start justify-between gap-4">
              <div class="space-y-1 max-w-md md:max-w-lg">
                <h4 class="text-xs font-semibold text-white">Offline Downloads Space</h4>
                <p class="text-xs text-[var(--text-secondary)] leading-relaxed">
                  Total storage consumed by offline downloaded movie and episode files.
                </p>
                <span class="text-xs app-pill px-2.5 py-0.5 inline-block font-mono">
                  {#if loadingStorage}
                    Calculating...
                  {:else if storageUsage}
                    {formatBytes(storageUsage.downloads_bytes)}
                  {:else}
                    --
                  {/if}
                </span>
              </div>
              <Button variant="danger" onclick={handleDeleteAllDownloads} disabled={deletingDownloads} className="shrink-0">
                <span class="text-xs font-semibold">{deletingDownloads ? "Wiping..." : "Delete All Downloads"}</span>
              </Button>
            </div>
          </div>
        </div>

      {:else if activeTab === "maintenance"}
        <!-- App Updates -->
        <div class="glass-panel rounded-2xl p-4">
          <div class="flex items-center justify-between mb-3">
            <h3 class="text-sm font-semibold">App Updates</h3>
            <span class="text-[11px] app-pill px-2 py-0.5">v{appVersion}</span>
          </div>

          {#if updaterStatus === "idle" || updaterStatus === "upToDate" || updaterStatus === "error"}
            <div class="flex flex-col gap-3">
              {#if updaterStatus === "upToDate"}
                <div class="flex items-center gap-2 text-sm text-emerald-300">
                  <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                    <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/>
                  </svg>
                  <span>You're on the latest version.</span>
                </div>
              {/if}

              {#if updaterStatus === "error" && updaterError}
                <div class="rounded-xl border border-red-300/30 bg-red-500/12 px-3 py-2.5 text-sm text-red-200">
                  <p class="font-medium">Update check failed</p>
                  <p class="text-xs app-faint mt-1 break-words">{updaterError}</p>
                </div>
              {/if}

              <Button variant="secondary" onclick={checkForUpdates}>
                <span class="text-sm">Check for Updates</span>
              </Button>
            </div>
          {:else if updaterStatus === "checking"}
            <div class="flex items-center gap-3 text-sm app-muted">
              <svg class="w-5 h-5 animate-spin text-cyan-400" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25" />
                <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
              </svg>
              <span>Checking for updates...</span>
            </div>
          {:else if updaterStatus === "available" && updaterInfo}
            <div class="flex flex-col gap-3">
              <div class="rounded-xl border border-cyan-400/25 bg-cyan-500/8 px-4 py-3">
                <div class="flex items-center gap-2 mb-2">
                  <svg class="w-5 h-5 text-cyan-300 shrink-0" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                    <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd"/>
                  </svg>
                  <span class="text-sm font-semibold text-cyan-100">Version {updaterInfo.version} is available</span>
                </div>

                {#if updaterInfo.body}
                  <div class="text-xs text-[var(--text-secondary)] space-y-1 max-h-40 overflow-y-auto pr-2 leading-relaxed whitespace-pre-wrap">
                    {updaterInfo.body}
                  </div>
                {/if}
              </div>

              <div class="flex gap-2">
                <Button variant="primary" onclick={installUpdate}>
                  <div class="flex items-center gap-1.5">
                    <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                      <path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm3.293-7.707a1 1 0 011.414 0L9 10.586V3a1 1 0 112 0v7.586l1.293-1.293a1 1 0 111.414 1.414l-3 3a1 1 0 01-1.414 0l-3-3a1 1 0 010-1.414z" clip-rule="evenodd"/>
                    </svg>
                    <span class="text-sm font-semibold">Install Update</span>
                  </div>
                </Button>
                <Button variant="secondary" onclick={dismissUpdate}>
                  <span class="text-sm">Later</span>
                </Button>
              </div>
            </div>
          {:else if updaterStatus === "downloading" || updaterStatus === "installing"}
            <div class="flex flex-col gap-2">
              <div class="flex items-center gap-3 text-sm">
                <svg class="w-5 h-5 animate-spin text-cyan-400 shrink-0" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                  <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" class="opacity-25" />
                  <path d="M4 12a8 8 0 018-8" stroke="currentColor" stroke-width="3" stroke-linecap="round" />
                </svg>
                <span class="text-[var(--text-secondary)]">
                  {updaterStatus === "installing" ? "Installing update..." : "Downloading update..."}
                </span>
              </div>
              {#if updaterProgress !== null}
                <div class="h-2 w-full bg-white/10 rounded-full overflow-hidden">
                  <div
                    class="h-full bg-gradient-to-r from-cyan-500 to-cyan-300 rounded-full transition-all duration-300"
                    style="width: {updaterProgress}%"
                  ></div>
                </div>
                <p class="text-xs app-faint text-right">{updaterProgress}%</p>
              {/if}
            </div>
          {:else if updaterStatus === "readyToRestart"}
            <div class="flex flex-col gap-3">
              <div class="flex items-center gap-2 text-sm text-emerald-300">
                <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                  <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/>
                </svg>
                <span>Update installed! Restart to apply.</span>
              </div>
              <Button variant="primary" onclick={performRestart}>
                <div class="flex items-center gap-1.5">
                  <svg class="w-4 h-4" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                    <path fill-rule="evenodd" d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z" clip-rule="evenodd"/>
                  </svg>
                  <span class="text-sm font-semibold">Restart Now</span>
                </div>
              </Button>
            </div>
          {/if}
        </div>

        <!-- Maintenance Tools -->
        <div class="glass-panel rounded-2xl p-5">
          <h3 class="text-sm font-semibold mb-4 text-white">System Diagnostics & Maintenance</h3>
          
          <div class="space-y-5">
            <!-- Diagnostics Option -->
            <div class="flex flex-col sm:flex-row sm:items-start justify-between gap-4 pb-4 border-b border-white/10">
              <div class="space-y-1 max-w-md md:max-w-lg">
                <h4 class="text-xs font-semibold text-white">Export Diagnostics</h4>
                <p class="text-xs text-[var(--text-secondary)] leading-relaxed">
                  Generates and exports a comprehensive diagnostics file containing runtime environment specifications, local database statistics, and the recent 250 log entries. This report helps identify synchronization, playback, or network issues, saving directly to your local support directory.
                </p>
              </div>
              <Button variant="secondary" onclick={handleDownloadDiagnostics} className="shrink-0">
                <span class="text-xs font-semibold">
                  {downloadingDiagnostics ? "Preparing..." : "Download Diagnostics"}
                </span>
              </Button>
            </div>

            <!-- Resync Option -->
            <div class="flex flex-col sm:flex-row sm:items-start justify-between gap-4 pb-4 border-b border-white/10">
              <div class="space-y-1 max-w-md md:max-w-lg">
                <h4 class="text-xs font-semibold text-white">Force Library Resync</h4>
                <p class="text-xs text-[var(--text-secondary)] leading-relaxed">
                  Clears the local SQLite database cache, resets checkpoints, and initiates a clean, full-library synchronization from the server. This fixes outdated catalogs or watch status errors. <strong class="text-amber-300 font-medium">Important:</strong> This does not delete any of your downloaded offline media files.
                </p>
              </div>
              <Button variant="secondary" onclick={handleResync} className="shrink-0">
                <span class="text-xs font-semibold">{runningResync ? "Resyncing..." : "Force Resync"}</span>
              </Button>
            </div>
          </div>
        </div>

        <!-- User Account Session -->
        <div class="glass-panel rounded-2xl p-5">
          <h3 class="text-sm font-semibold mb-4 text-white">Account Session</h3>
          <div class="flex flex-col sm:flex-row sm:items-start justify-between gap-4">
            <div class="space-y-1 max-w-md md:max-w-lg">
              <h4 class="text-xs font-semibold text-white">Log Out Session</h4>
              <p class="text-xs text-[var(--text-secondary)] leading-relaxed">
                Ends the active session on this device, clears in-memory library synchronization checkpoints, and returns to the connection screen.
              </p>
            </div>
            <Button variant="danger" onclick={handleLogout} className="shrink-0">
              <span class="text-xs font-semibold">Log Out</span>
            </Button>
          </div>
        </div>

      {:else if activeTab === "about"}
        <div class="glass-panel rounded-2xl p-6 text-center max-w-lg mx-auto">
          <div class="w-16 h-16 rounded-2xl bg-[linear-gradient(135deg,#66d8ff_0%,#41b8d5_54%,#f4bc6b_100%)] flex items-center justify-center text-slate-950 shadow-[0_12px_32px_rgba(65,184,213,0.3)] mx-auto mb-4">
            <svg class="w-9 h-9" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 14.5v-9l6 4.5-6 4.5z"/>
            </svg>
          </div>
          <h3 class="text-lg font-semibold text-white mb-1">jfFast</h3>
          <p class="text-xs app-badge px-2.5 py-0.5 inline-block mb-4">v{appVersion}</p>
          
          <p class="text-sm text-[var(--text-secondary)] leading-relaxed mb-6">
            A lightweight, fast, and feature-rich desktop client for Jellyfin. Designed for high performance, smooth playback, and native offline synchronization capabilities.
          </p>

          <div class="grid grid-cols-2 gap-4 text-left border-t border-white/10 pt-4 text-xs">
            <div>
              <span class="app-faint block">Tauri Framework</span>
              <span class="text-[var(--text-secondary)] font-medium">v2.x</span>
            </div>
            <div>
              <span class="app-faint block">App Version</span>
              <span class="text-[var(--text-secondary)] font-medium">{appVersion}</span>
            </div>
            <div>
              <span class="app-faint block">Local Database</span>
              <span class="text-[var(--text-secondary)] font-medium">SQLite</span>
            </div>
            <div>
              <span class="app-faint block">Video Engine</span>
              <span class="text-[var(--text-secondary)] font-medium">MPV (DirectPlay/Transcode)</span>
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>
</section>
