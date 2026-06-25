<script lang="ts">
  import {
    getSyncState,
    getSyncProgress,
    getFatalError,
    getSyncErrors,
  } from "../../lib/stores/sync.svelte";
  import {
    isOnline,
    isDegraded,
    getLastNetworkError,
  } from "../../lib/stores/connectivity.svelte";
  import { pushToast, pushErrorToast } from "../../lib/stores/toast.svelte";
  import { fade } from "svelte/transition";

  const syncState = $derived(getSyncState());
  const progress = $derived(getSyncProgress());
  const fatalError = $derived(getFatalError());
  const syncErrors = $derived(getSyncErrors());
  const online = $derived(isOnline());
  const degraded = $derived(isDegraded());
  const lastNetworkError = $derived(getLastNetworkError());

  let lastObservedErrorCount = $state(0);
  let lastFatalNotified = $state("");

  $effect(() => {
    if (syncErrors.length <= lastObservedErrorCount) return;

    const newErrors = syncErrors.slice(lastObservedErrorCount);
    lastObservedErrorCount = syncErrors.length;

    for (const err of newErrors) {
      if (err.is_fatal) continue;
      pushToast({
        level: "warning",
        source: "sync",
        title: "Sync warning",
        message: err.message,
        dismissAfterMs: 5000,
        dedupeKey: `sync-warning-${err.batch_start}-${err.message}`,
      });
    }
  });

  $effect(() => {
    if (syncState !== "complete_with_errors") return;

    const message = fatalError || "Library indexed with errors";
    if (message === lastFatalNotified) return;

    lastFatalNotified = message;
    pushErrorToast("sync", message, "Sync failed", `sync-fatal-${message}`);
  });
</script>

{#if degraded}
  <div
    transition:fade={{ duration: 200 }}
    class="fixed top-0 left-0 right-0 z-[60]"
  >
    <div class="bg-orange-900/90 backdrop-blur-sm border-b border-orange-700 px-4 py-2">
      <div class="flex items-center gap-3 max-w-screen-lg mx-auto">
        <svg class="w-4 h-4 text-orange-300 shrink-0" viewBox="0 0 20 20" fill="currentColor">
          <path
            fill-rule="evenodd"
            d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z"
            clip-rule="evenodd"
          />
        </svg>
        <span class="text-sm text-orange-200">
          Degraded network state. {lastNetworkError || "Some live requests failed."}
        </span>
      </div>
    </div>
  </div>
{/if}

{#if syncState === "syncing" && progress}
  <div
    transition:fade={{ duration: 300 }}
    class="fixed bottom-0 left-0 right-0 z-50"
  >
    <div class="bg-gray-800/90 backdrop-blur-sm border-t border-gray-700 px-4 py-2">
      <div class="flex items-center gap-3 max-w-screen-lg mx-auto">
        <svg
          class="w-4 h-4 text-blue-400 animate-spin shrink-0"
          viewBox="0 0 24 24"
          fill="none"
        >
          <circle
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            stroke-width="3"
            class="opacity-25"
          />
          <path
            d="M4 12a8 8 0 018-8"
            stroke="currentColor"
            stroke-width="3"
            stroke-linecap="round"
          />
        </svg>

        <span class="text-sm text-gray-300">
          Indexing library... {progress.percentage.toFixed(0)}%
        </span>

        <div class="flex-1 h-1.5 bg-gray-700 rounded-full overflow-hidden">
          <div
            class="h-full bg-blue-500 rounded-full transition-all duration-300"
            style="width: {progress.percentage}%"
          ></div>
        </div>

        <span class="text-xs text-gray-500 shrink-0">
          {progress.current.toLocaleString()} / {progress.total.toLocaleString()}
        </span>
      </div>
    </div>
  </div>
{:else if syncState === "complete"}
  <div
    transition:fade={{ duration: 300 }}
    class="fixed bottom-0 left-0 right-0 z-50"
  >
    <div class="bg-green-900/80 backdrop-blur-sm border-t border-green-700 px-4 py-2">
      <div class="flex items-center gap-3 max-w-screen-lg mx-auto">
        <svg class="w-4 h-4 text-green-400 shrink-0" viewBox="0 0 20 20" fill="currentColor">
          <path
            fill-rule="evenodd"
            d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
            clip-rule="evenodd"
          />
        </svg>
        <span class="text-sm text-green-300">Library indexed successfully</span>
      </div>
    </div>
  </div>
{:else if syncState === "complete_with_errors"}
  <div
    transition:fade={{ duration: 300 }}
    class="fixed bottom-0 left-0 right-0 z-50"
  >
    <div class="bg-yellow-900/80 backdrop-blur-sm border-t border-yellow-700 px-4 py-2">
      <div class="flex items-center gap-3 max-w-screen-lg mx-auto">
        <svg class="w-4 h-4 text-yellow-400 shrink-0" viewBox="0 0 20 20" fill="currentColor">
          <path
            fill-rule="evenodd"
            d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z"
            clip-rule="evenodd"
          />
        </svg>
        <span class="text-sm text-yellow-300">
          {fatalError || "Library indexed with some errors"}
        </span>
      </div>
    </div>
  </div>
{/if}
