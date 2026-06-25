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
  } from "../../lib/api";
  import { resetSyncStore } from "../../lib/stores/sync.svelte";
  import { setUnauthenticated } from "../../lib/stores/auth.svelte";
  import Button from "../../components/ui/Button.svelte";

  interface Props {
    onResync: () => Promise<void>;
  }

  let { onResync }: Props = $props();

  let activeRouteHeading = $state<HTMLElement | null>(null);

  const preferences = $derived(getPreferences());
  const preferencesLoaded = $derived(isPreferencesLoaded());
  const preferencesSaving = $derived(isPreferencesSaving());

  const online = $derived(isOnline());

  const updaterStatus = $derived(getUpdateStatus());
  const updaterInfo = $derived(getUpdateInfo());
  const updaterProgress = $derived(getDownloadProgress());
  const updaterError = $derived(getUpdateError());

  let appVersion = $state("...");
  let downloadingDiagnostics = $state(false);
  let runningResync = $state(false);

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

    void tick().then(() => {
      activeRouteHeading?.focus();
    });
  });

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
      await onResync();
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

<section class="px-6 pt-6 pb-10 max-w-3xl app-animate-fade-up" aria-label="Settings">
  <div class="flex flex-wrap items-center justify-between gap-3 mb-4">
    <h2 bind:this={activeRouteHeading} tabindex="-1" class="text-xl font-semibold focus:outline-none">Preferences</h2>
    {#if preferencesSaving}
      <span class="text-xs app-badge px-2 py-1">Saving...</span>
    {/if}
  </div>

  <div class="space-y-4">
    <div class="glass-panel rounded-2xl p-4">
      <h3 class="text-sm font-semibold mb-3">Playback</h3>
      <div class="grid gap-3 md:grid-cols-2">
        <label class="text-sm text-[var(--text-secondary)] flex items-center gap-2">
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

        <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1 md:col-span-2">
          Subtitle position
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
      </div>
    </div>

    <div class="glass-panel rounded-2xl p-4">
      <h3 class="text-sm font-semibold mb-3">Language + Quality</h3>
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
      <h3 class="text-sm font-semibold mb-3">Cache + Refresh</h3>
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

        <label class="text-sm text-[var(--text-secondary)] flex flex-col gap-1">
          Refresh interval
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
          <span class="text-[11px] app-faint">Select where offline movies and episodes will be stored.</span>
        </div>
      </div>
    </div>

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

    <div class="glass-panel rounded-2xl p-4">
      <h3 class="text-sm font-semibold mb-2">Maintenance</h3>
      <div class="flex flex-wrap gap-3">
        <Button variant="secondary" onclick={handleDownloadDiagnostics}>
          <span class="text-sm">
            {downloadingDiagnostics ? "Preparing Diagnostics..." : "Download Diagnostics"}
          </span>
        </Button>
        <Button variant="secondary" onclick={handleResync}>
          <span class="text-sm">{runningResync ? "Resyncing..." : "Force Resync"}</span>
        </Button>
        <Button variant="secondary" onclick={handleLogout}>
          <span class="text-sm">Log Out</span>
        </Button>
      </div>
    </div>
  </div>
</section>
