<script lang="ts">
  import {
    isPlayerVisible,
    getPlayerStatus,
    getPlayerTitle,
    getTimePos,
    getDuration,
    getVolume,
    hidePlayer,
    setVolume,
  } from "../../lib/stores/player.svelte";
  import {
    mpvTogglePause,
    mpvSeek,
    mpvSeekAbsolute,
    mpvSetVolume,
    mpvStop,
  } from "../../lib/api";

  // ── Auto-hide logic ──────────────────────────────────────────
  let controlsVisible = $state(true);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  function resetHideTimer() {
    controlsVisible = true;
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      controlsVisible = false;
    }, 3000);
  }

  function handleMouseMove() {
    resetHideTimer();
  }

  // ── Derived values ───────────────────────────────────────────
  const playerVisible = $derived(isPlayerVisible());
  const playerStatus = $derived(getPlayerStatus());
  const playerTitle = $derived(getPlayerTitle());
  const pos = $derived(getTimePos());
  const dur = $derived(getDuration());
  const vol = $derived(getVolume());
  const progressPercent = $derived(dur > 0 ? (pos / dur) * 100 : 0);
  const isPaused = $derived(playerStatus === "paused");

  // ── Formatting helpers ───────────────────────────────────────
  function formatTime(seconds: number): string {
    if (!seconds || seconds < 0) return "0:00";
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    const pad = (n: number) => n.toString().padStart(2, "0");
    return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
  }

  function endTimeEstimate(): string {
    const remaining = dur - pos;
    if (remaining <= 0) return "";
    const end = new Date(Date.now() + remaining * 1000);
    return `Ends at ${end.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  }

  // ── Control handlers ─────────────────────────────────────────
  async function togglePause() {
    await mpvTogglePause();
  }

  async function seekBack10() {
    await mpvSeek(-10);
  }

  async function seekForward30() {
    await mpvSeek(30);
  }

  async function handleProgressClick(e: MouseEvent) {
    const bar = e.currentTarget as HTMLElement;
    const rect = bar.getBoundingClientRect();
    const fraction = (e.clientX - rect.left) / rect.width;
    const targetSeconds = fraction * dur;
    await mpvSeekAbsolute(targetSeconds);
  }

  async function handleVolumeInput(e: Event) {
    const v = parseFloat((e.target as HTMLInputElement).value);
    setVolume(v);
    await mpvSetVolume(v);
  }

  async function stopPlayer() {
    await mpvStop();
    hidePlayer();
  }

  // ── Keyboard shortcuts ───────────────────────────────────────
  function handleKeydown(e: KeyboardEvent) {
    if (!playerVisible) return;
    switch (e.key) {
      case " ":
        e.preventDefault();
        togglePause();
        break;
      case "ArrowLeft":
        mpvSeek(-10);
        break;
      case "ArrowRight":
        mpvSeek(30);
        break;
      case "ArrowUp": {
        e.preventDefault();
        const newVolUp = Math.min(vol + 5, 100);
        setVolume(newVolUp);
        mpvSetVolume(newVolUp);
        break;
      }
      case "ArrowDown": {
        e.preventDefault();
        const newVolDown = Math.max(vol - 5, 0);
        setVolume(newVolDown);
        mpvSetVolume(newVolDown);
        break;
      }
      case "Escape":
        stopPlayer();
        break;
    }
    resetHideTimer();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if playerVisible}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-[9999] flex flex-col justify-between"
    class:cursor-none={!controlsVisible}
    onmousemove={handleMouseMove}
  >
    <!-- ═══ TOP BAR ═══ -->
    <div
      class="relative z-10 px-5 pt-4 pb-12 bg-gradient-to-b from-black/80 to-transparent
             transition-all duration-300 ease-out"
      class:opacity-0={!controlsVisible}
      class:-translate-y-full={!controlsVisible}
    >
      <div class="flex items-center gap-3">
        <button
          onclick={stopPlayer}
          aria-label="Close player"
          class="p-2 rounded-lg hover:bg-white/10 transition-colors"
        >
          <svg
            class="w-5 h-5 text-white"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              d="M15 19l-7-7 7-7"
            />
          </svg>
        </button>
        <h2 class="text-white text-lg font-bold truncate">{playerTitle}</h2>
      </div>
    </div>

    <!-- ═══ CENTER CLICK ZONE (transparent — video shows through) ═══ -->
    <button
      class="absolute inset-0 z-0 w-full h-full"
      onclick={togglePause}
      aria-label={isPaused ? "Resume playback" : "Pause playback"}
    ></button>

    <!-- ═══ BOTTOM CONTROLS ═══ -->
    <div
      class="relative z-10 px-5 pb-4 pt-16 bg-gradient-to-t from-black/80 to-transparent
             transition-all duration-300 ease-out"
      class:opacity-0={!controlsVisible}
      class:translate-y-full={!controlsVisible}
    >
      <!-- Info row -->
      <div
        class="flex items-center justify-between mb-2 text-xs text-gray-300"
      >
        <span class="truncate max-w-[60%] font-semibold"
          >{playerTitle}
          {#if endTimeEstimate()}
            <span class="font-normal text-gray-400">
              — {endTimeEstimate()}</span
            >
          {/if}
        </span>
        <div class="flex items-center gap-2">
          <span
            class="text-xs text-blue-400 bg-blue-400/10 px-1.5 py-0.5 rounded font-medium"
          >
            Direct Play
          </span>
        </div>
      </div>

      <!-- Progress bar -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="group w-full h-4 flex items-center mb-3 cursor-pointer"
        onclick={handleProgressClick}
        role="slider"
        aria-label="Seek in video"
        aria-valuenow={Math.floor(pos)}
        aria-valuemin={0}
        aria-valuemax={Math.floor(dur)}
        tabindex="-1"
      >
        <div
          class="w-full h-1 group-hover:h-1.5 bg-white/20 rounded-full transition-all relative overflow-hidden"
        >
          <div
            class="absolute top-0 left-0 h-full bg-blue-500 rounded-full"
            style="width: {progressPercent}%"
          ></div>
        </div>
      </div>

      <!-- Controls row -->
      <div class="flex items-center justify-between">
        <!-- Left: current time -->
        <div class="flex items-center gap-3 text-sm text-gray-300 min-w-0">
          <span class="text-white font-medium tabular-nums"
            >{formatTime(pos)}</span
          >
        </div>

        <!-- Center: playback controls -->
        <div class="flex items-center gap-2">
          <!-- Seek back 10s -->
          <button
            onclick={seekBack10}
            aria-label="Seek back 10 seconds"
            class="p-2 rounded-lg hover:bg-white/10 transition-colors text-white"
          >
            <svg class="w-6 h-6" viewBox="0 0 24 24" fill="currentColor">
              <path
                d="M12.5 8C9.85 8 7.45 9.11 5.66 10.86L2 7.2V16h8.8l-3.66-3.66C8.65 10.81 10.48 10 12.5 10c3.87 0 7.05 3.07 7.18 6.92l1.98-.22C21.45 12.4 17.38 8 12.5 8z"
              />
              <text
                x="10"
                y="19"
                font-size="7"
                font-weight="bold"
                fill="currentColor">10</text
              >
            </svg>
          </button>

          <!-- Play/Pause -->
          <button
            onclick={togglePause}
            aria-label={isPaused ? "Play" : "Pause"}
            class="p-3 rounded-full bg-white/10 hover:bg-white/20 transition-colors text-white"
          >
            {#if isPaused || playerStatus === "loading"}
              <svg class="w-7 h-7" viewBox="0 0 24 24" fill="currentColor">
                <path d="M8 5v14l11-7z" />
              </svg>
            {:else}
              <svg class="w-7 h-7" viewBox="0 0 24 24" fill="currentColor">
                <path d="M6 4h4v16H6zM14 4h4v16h-4z" />
              </svg>
            {/if}
          </button>

          <!-- Seek forward 30s -->
          <button
            onclick={seekForward30}
            aria-label="Seek forward 30 seconds"
            class="p-2 rounded-lg hover:bg-white/10 transition-colors text-white"
          >
            <svg class="w-6 h-6" viewBox="0 0 24 24" fill="currentColor">
              <path
                d="M11.5 8c2.65 0 5.05 1.11 6.84 2.86L22 7.2V16h-8.8l3.66-3.66C15.35 10.81 13.52 10 11.5 10c-3.87 0-7.05 3.07-7.18 6.92l-1.98-.22C2.55 12.4 6.62 8 11.5 8z"
              />
              <text
                x="8"
                y="19"
                font-size="7"
                font-weight="bold"
                fill="currentColor">30</text
              >
            </svg>
          </button>
        </div>

        <!-- Right: remaining time + volume -->
        <div class="flex items-center gap-3 text-sm text-gray-300 min-w-0">
          <span class="tabular-nums">-{formatTime(dur - pos)}</span>
          <div class="flex items-center gap-1.5">
            <svg
              class="w-4 h-4 text-gray-400"
              viewBox="0 0 20 20"
              fill="currentColor"
            >
              <path
                fill-rule="evenodd"
                d="M9.383 3.076A1 1 0 0110 4v12a1 1 0 01-1.707.707L4.586 13H2a1 1 0 01-1-1V8a1 1 0 011-1h2.586l3.707-3.707a1 1 0 011.09-.217zM14.657 2.929a1 1 0 011.414 0A9.972 9.972 0 0119 10a9.972 9.972 0 01-2.929 7.071 1 1 0 01-1.414-1.414A7.971 7.971 0 0017 10c0-2.21-.894-4.208-2.343-5.657a1 1 0 010-1.414z"
                clip-rule="evenodd"
              />
            </svg>
            <input
              type="range"
              min="0"
              max="100"
              step="1"
              value={vol}
              oninput={handleVolumeInput}
              aria-label="Volume"
              class="w-20 h-1 accent-blue-500 cursor-pointer"
            />
            <span class="text-xs w-6 text-right tabular-nums">{vol}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}
