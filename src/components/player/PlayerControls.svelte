<script lang="ts">
  import type { MediaStreamInfo, MediaItem } from "../../lib/types";
  import type { QualityOption } from "../../lib/mediaStreamHelpers";
  import PlayerAutoplay from "./PlayerAutoplay.svelte";

  let {
    children,
    playerTitle,
    selectedQualityLabel,
    endTimeEstimate,
    mediaStreams,
    audioMenuLabel,
    subtitleMenuLabel,
    audioMenuOpen,
    subtitleMenuOpen,
    overflowMenuOpen,
    toggleTopMenu,
    selectedAudioIndex,
    selectedSubtitleIndex,
    applyTrackSelection,
    playbackRate,
    mpvSetPlaybackRate,
    qualityOptions,
    selectedQualityKey,
    changeQuality,
    autoplayCountdown,
    cancelAutoplayCountdown,
    formatTime,
    effectivePos,
    dur,
    previousEpisode,
    nextEpisode,
    playPreviousEpisode,
    playNextEpisode,
    seekBack10,
    seekForward30,
    togglePause,
    isPaused,
    playerStatus,
    vol,
    handleVolumeInput,
    toggleMute,
    muted,
    controlsVisible,
  }: {
    children?: import('svelte').Snippet;
    playerTitle: string;
    selectedQualityLabel: string;
    endTimeEstimate: () => string;
    mediaStreams: MediaStreamInfo | null;
    audioMenuLabel: string;
    subtitleMenuLabel: string;
    audioMenuOpen: boolean;
    subtitleMenuOpen: boolean;
    overflowMenuOpen: boolean;
    toggleTopMenu: (menu: "audio" | "subtitle" | "overflow") => void;
    selectedAudioIndex: number | null;
    selectedSubtitleIndex: number | null;
    applyTrackSelection: (nextAudioIndex: number | null, nextSubtitleIndex: number | null) => Promise<void>;
    playbackRate: number;
    mpvSetPlaybackRate: (v: number) => Promise<void>;
    qualityOptions: QualityOption[];
    selectedQualityKey: string;
    changeQuality: (key: string) => Promise<void>;
    autoplayCountdown: number | null;
    cancelAutoplayCountdown: () => void;
    formatTime: (seconds: number) => string;
    effectivePos: number;
    dur: number;
    previousEpisode: MediaItem | null;
    nextEpisode: MediaItem | null;
    playPreviousEpisode: () => Promise<void>;
    playNextEpisode: () => Promise<void>;
    seekBack10: () => Promise<void>;
    seekForward30: () => Promise<void>;
    togglePause: () => Promise<void>;
    isPaused: boolean;
    playerStatus: string;
    vol: number;
    handleVolumeInput: (e: Event) => Promise<void>;
    toggleMute: () => Promise<void>;
    muted: boolean;
    controlsVisible: boolean;
  } = $props();

  const playbackSpeeds = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3];
</script>

<div
  class="relative z-10 px-3 sm:px-6 pb-2 sm:pb-3 pt-10 sm:pt-12 bg-gradient-to-t from-[rgba(3,6,12,0.92)] via-[rgba(5,9,17,0.62)] to-transparent transition-all duration-300 ease-out"
  class:opacity-0={!controlsVisible}
  class:translate-y-full={!controlsVisible}
>
  <div class="mx-auto w-full max-w-6xl rounded-2xl border border-white/22 bg-[linear-gradient(145deg,rgba(255,255,255,0.16),rgba(255,255,255,0.05))] backdrop-blur-2xl shadow-[0_20px_42px_rgba(2,8,20,0.58)] p-2.5 sm:p-3">
    <div class="flex flex-wrap items-center justify-between gap-2 sm:gap-2.5 mb-2">
      <div class="min-w-0">
        <p class="text-white text-sm font-semibold truncate">{playerTitle}</p>
        <div class="flex flex-wrap items-center gap-1.5 mt-1 text-[10px] sm:text-[11px]">
          <span class="px-2 py-0.5 rounded-full bg-[rgba(102,216,255,0.16)] text-cyan-100 border border-cyan-200/25">{selectedQualityLabel}</span>
          {#if endTimeEstimate()}
            <span class="text-slate-300">{endTimeEstimate()}</span>
          {/if}
        </div>
      </div>

      <div class="flex items-center gap-2 flex-wrap justify-end">
        <div class="flex items-center gap-1.5">
          {#if mediaStreams && mediaStreams.audio.length > 0}
            <div class="relative">
              <button
                onclick={() => toggleTopMenu("audio")}
                aria-label="Select audio language"
                class="h-8 px-2.5 rounded-lg bg-[rgba(8,16,28,0.7)] border border-white/24 backdrop-blur-xl text-white hover:bg-[rgba(22,35,55,0.78)] transition-colors inline-flex items-center gap-1.5 max-w-[170px]"
              >
                <span class="text-[10px] uppercase tracking-wide text-cyan-200">Lang</span>
                <span class="text-[11px] truncate">{audioMenuLabel}</span>
              </button>

              {#if audioMenuOpen}
                <div class="absolute right-0 bottom-10 w-64 max-h-72 overflow-auto rounded-xl border border-white/22 bg-[rgba(7,14,24,0.88)] backdrop-blur-2xl shadow-[0_20px_50px_rgba(0,0,0,0.58)] p-1.5">
                  {#each mediaStreams.audio as track}
                    <button
                      onclick={() => {
                        void applyTrackSelection(track.index, selectedSubtitleIndex);
                        toggleTopMenu("audio");
                      }}
                      class="w-full text-left px-3 py-2 rounded-lg text-xs hover:bg-white/14 transition-colors {selectedAudioIndex === track.index ? 'bg-cyan-500/30 text-cyan-100' : 'text-gray-100'}"
                    >
                      {track.display_title}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}

          <div class="relative">
            <button
              onclick={() => toggleTopMenu("subtitle")}
              aria-label="Select subtitles"
              class="h-8 px-2.5 rounded-lg bg-[rgba(8,16,28,0.7)] border border-white/24 backdrop-blur-xl text-white hover:bg-[rgba(22,35,55,0.78)] transition-colors inline-flex items-center gap-1.5 max-w-[170px]"
            >
              <span class="text-[10px] uppercase tracking-wide text-emerald-200">Subs</span>
              <span class="text-[11px] truncate">{subtitleMenuLabel}</span>
            </button>

            {#if subtitleMenuOpen}
              <div class="absolute right-0 bottom-10 w-64 max-h-72 overflow-auto rounded-xl border border-white/22 bg-[rgba(7,14,24,0.88)] backdrop-blur-2xl shadow-[0_20px_50px_rgba(0,0,0,0.58)] p-1.5">
                <button
                  onclick={() => {
                    void applyTrackSelection(selectedAudioIndex, null);
                    toggleTopMenu("subtitle");
                  }}
                  class="w-full text-left px-3 py-2 rounded-lg text-xs hover:bg-white/14 transition-colors {selectedSubtitleIndex === null ? 'bg-cyan-500/30 text-cyan-100' : 'text-gray-100'}"
                >
                  Off
                </button>
                {#if mediaStreams}
                  {#each mediaStreams.subtitle as track}
                    <button
                      onclick={() => {
                        void applyTrackSelection(selectedAudioIndex, track.index);
                        toggleTopMenu("subtitle");
                      }}
                      class="w-full text-left px-3 py-2 rounded-lg text-xs hover:bg-white/14 transition-colors {selectedSubtitleIndex === track.index ? 'bg-cyan-500/30 text-cyan-100' : 'text-gray-100'}"
                    >
                      {track.display_title}
                    </button>
                  {/each}
                {/if}
              </div>
            {/if}
          </div>

          <div class="relative">
            <button
              onclick={() => toggleTopMenu("overflow")}
              aria-label="More options"
              class="h-8 w-8 grid place-items-center rounded-lg bg-[rgba(8,16,28,0.7)] border border-white/24 backdrop-blur-xl text-white hover:bg-[rgba(22,35,55,0.78)] transition-colors"
            >
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
              </svg>
            </button>

            {#if overflowMenuOpen}
              <div class="absolute right-0 bottom-10 w-56 rounded-xl border border-white/22 bg-[rgba(7,14,24,0.88)] backdrop-blur-2xl shadow-[0_20px_50px_rgba(0,0,0,0.58)] p-2 space-y-3">
                <div>
                  <p class="px-2 pb-1 text-[11px] uppercase tracking-wide text-gray-400">Playback Speed</p>
                  <div class="grid grid-cols-3 gap-1">
                    {#each playbackSpeeds as speed}
                      <button
                        onclick={() => {
                          void mpvSetPlaybackRate(speed);
                          toggleTopMenu("overflow");
                        }}
                        class="h-8 rounded-lg text-xs text-gray-100 border hover:bg-white/15 transition-colors {playbackRate === speed ? 'bg-cyan-500/35 border-cyan-400' : 'border-white/18'}"
                      >
                        {speed}x
                      </button>
                    {/each}
                  </div>
                </div>

                <div>
                  <p class="px-2 pb-1 text-[11px] uppercase tracking-wide text-gray-400">Quality</p>
                  <div class="grid grid-cols-2 gap-1">
                    {#each qualityOptions as option}
                      <button
                        onclick={() => {
                          void changeQuality(option.key);
                          toggleTopMenu("overflow");
                        }}
                        class="h-8 rounded-lg text-xs text-gray-100 border hover:bg-white/15 transition-colors {selectedQualityKey === option.key ? 'bg-cyan-500/35 border-cyan-400' : 'border-white/18'}"
                      >
                        {option.label.split(" · ")[0]}
                      </button>
                    {/each}
                  </div>
                </div>
              </div>
            {/if}
          </div>

          {#if autoplayCountdown !== null}
            <PlayerAutoplay {autoplayCountdown} {cancelAutoplayCountdown} />
          {/if}
        </div>
      </div>
    </div>

    {#if children}
      {@render children()}
    {/if}

    <div class="grid grid-cols-1 sm:grid-cols-[auto_1fr_auto] gap-2 items-center mb-1">
      <div class="text-sm text-gray-300">
        <span class="text-white font-medium tabular-nums">{formatTime(effectivePos)}</span>
        <span class="text-gray-400 ml-1">/ {formatTime(dur)}</span>
      </div>

      <div class="flex items-center justify-center gap-1.5 sm:gap-2">
        {#if previousEpisode}
          <button
            onclick={playPreviousEpisode}
            aria-label="Play previous episode"
            class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/15 text-white hover:bg-white/20 transition-colors"
          >
            <svg class="w-4.5 h-4.5" viewBox="0 0 24 24" fill="currentColor">
              <path d="M20.5 6v12l-8.3-6z" />
              <path d="M12.5 6v12l-8.3-6z" />
            </svg>
          </button>
        {/if}

        <button
          onclick={seekBack10}
          aria-label="Seek back 10 seconds"
          class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/20 text-white hover:bg-white/18 transition-colors"
        >
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
            <path d="M11.92 5.08L4 12l7.92 6.92L10.5 20.5 1 12l9.5-8.5z" />
          </svg>
        </button>

        <button
          onclick={togglePause}
          aria-label={isPaused ? "Play" : "Pause"}
          class="h-11 w-11 sm:h-12 sm:w-12 grid place-items-center rounded-full bg-gradient-to-br from-cyan-200 via-sky-300 to-amber-300 text-slate-950 shadow-[0_10px_26px_rgba(102,216,255,0.38)] hover:scale-[1.03] transition-transform"
        >
          {#if isPaused || playerStatus === "loading"}
            <svg class="w-7 h-7" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z" />
            </svg>
          {:else}
            <svg class="w-6 h-6" viewBox="0 0 24 24" fill="currentColor">
              <path d="M6 4h4v16H6zM14 4h4v16h-4z" />
            </svg>
          {/if}
        </button>

        <button
          onclick={seekForward30}
          aria-label="Seek forward 30 seconds"
          class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/20 text-white hover:bg-white/18 transition-colors"
        >
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12.08 5.08L20 12l-7.92 6.92L13.5 20.5 23 12l-9.5-8.5z" />
          </svg>
        </button>

        {#if nextEpisode}
          <button
            onclick={playNextEpisode}
            aria-label="Play next episode"
            class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/15 text-white hover:bg-white/20 transition-colors"
          >
            <svg class="w-4.5 h-4.5" viewBox="0 0 24 24" fill="currentColor">
              <path d="M3.5 6v12l8.3-6z" />
              <path d="M11.5 6v12l8.3-6z" />
            </svg>
          </button>
        {/if}
      </div>

      <div class="flex items-center justify-end gap-1.5 text-sm text-gray-300">
        <button
          type="button"
          onclick={toggleMute}
          aria-label={muted ? "Unmute" : "Mute"}
          class="h-9 w-9 grid place-items-center rounded-lg bg-white/10 border border-white/20 hover:bg-white/18 transition-colors"
        >
          {#if muted}
            <svg class="w-4 h-4 text-gray-300" viewBox="0 0 20 20" fill="currentColor">
              <path d="M9.383 3.076A1 1 0 0110 4v12a1 1 0 01-1.707.707L4.586 13H2a1 1 0 01-1-1V8a1 1 0 011-1h2.586l3.707-3.707a1 1 0 011.09-.217z"/>
              <path d="M13.293 7.293a1 1 0 011.414 0L16 8.586l1.293-1.293a1 1 0 011.414 1.414L17.414 10l1.293 1.293a1 1 0 01-1.414 1.414L16 11.414l-1.293 1.293a1 1 0 01-1.414-1.414L14.586 10l-1.293-1.293a1 1 0 010-1.414z"/>
            </svg>
          {:else}
            <svg class="w-4 h-4 text-gray-300" viewBox="0 0 20 20" fill="currentColor">
              <path fill-rule="evenodd" d="M9.383 3.076A1 1 0 0110 4v12a1 1 0 01-1.707.707L4.586 13H2a1 1 0 01-1-1V8a1 1 0 011-1h2.586l3.707-3.707a1 1 0 011.09-.217zM14.657 2.929a1 1 0 011.414 0A9.972 9.972 0 0119 10a9.972 9.972 0 01-2.929 7.071 1 1 0 01-1.414-1.414A7.971 7.971 0 0017 10c0-2.21-.894-4.208-2.343-5.657a1 1 0 010-1.414z" clip-rule="evenodd"/>
            </svg>
          {/if}
        </button>

        <input
          type="range"
          min="0"
          max="100"
          step="1"
          value={vol}
          oninput={handleVolumeInput}
          aria-label="Volume"
          class="w-24 h-1.5 accent-[#66d8ff] cursor-pointer"
        />
        <span class="text-xs w-7 text-right tabular-nums">{vol}</span>
      </div>
    </div>
  </div>
</div>
