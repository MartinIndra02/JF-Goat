<script lang="ts">
  type Quality = "original" | "720" | "480" | "360";

  interface Props {
    onClose: () => void;
    onConfirm: (quality: Quality) => void;
  }

  let { onClose, onConfirm }: Props = $props();
  let downloadQualitySelection = $state<Quality>("original");
</script>

<!-- Modal Backdrop -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  onclick={onClose}
  class="fixed inset-0 z-[100] flex items-center justify-center p-4 bg-black/60 backdrop-blur-md"
>
  <!-- Modal Container -->
  <div
    onclick={(e) => e.stopPropagation()}
    class="w-full max-w-md bg-[rgba(10,18,31,0.92)] border border-white/20 rounded-2xl shadow-2xl backdrop-blur-2xl overflow-hidden max-h-[85vh] flex flex-col"
  >
    <!-- Modal Header -->
    <div class="px-5 py-4 border-b border-white/10 flex items-center justify-between">
      <h3 class="text-base font-bold text-white">Download Options</h3>
      <button
        onclick={onClose}
        class="text-gray-400 hover:text-white transition-colors"
        aria-label="Close dialog"
      >
        <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>

    <!-- Modal Body -->
    <div class="p-5 space-y-5 overflow-y-auto flex-1">
      <!-- Quality / Transcode option -->
      <div>
        <span class="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">Quality / Size</span>
        <div class="space-y-2">
          <label class="flex items-center gap-3 p-3 rounded-xl border border-white/10 bg-white/5 hover:bg-white/8 transition-all cursor-pointer">
            <input
              type="radio"
              name="download-quality"
              value="original"
              bind:group={downloadQualitySelection}
              class="accent-cyan-400"
            />
            <div class="min-w-0">
              <p class="text-sm font-semibold text-white">Original Quality</p>
              <p class="text-xs text-gray-400">Download the source file directly without quality loss</p>
            </div>
          </label>

          <label class="flex items-center gap-3 p-3 rounded-xl border border-white/10 bg-white/5 hover:bg-white/8 transition-all cursor-pointer">
            <input
              type="radio"
              name="download-quality"
              value="720"
              bind:group={downloadQualitySelection}
              class="accent-cyan-400"
            />
            <div class="min-w-0">
              <p class="text-sm font-semibold text-white">720p (Transcoded)</p>
              <p class="text-xs text-gray-400">Smaller file size (~4.5 Mbps, optimized for mobile devices)</p>
            </div>
          </label>

          <label class="flex items-center gap-3 p-3 rounded-xl border border-white/10 bg-white/5 hover:bg-white/8 transition-all cursor-pointer">
            <input
              type="radio"
              name="download-quality"
              value="480"
              bind:group={downloadQualitySelection}
              class="accent-cyan-400"
            />
            <div class="min-w-0">
              <p class="text-sm font-semibold text-white">480p (Transcoded)</p>
              <p class="text-xs text-gray-400">Very small file size (~2.0 Mbps, standard definition)</p>
            </div>
          </label>

          <label class="flex items-center gap-3 p-3 rounded-xl border border-white/10 bg-white/5 hover:bg-white/8 transition-all cursor-pointer">
            <input
              type="radio"
              name="download-quality"
              value="360"
              bind:group={downloadQualitySelection}
              class="accent-cyan-400"
            />
            <div class="min-w-0">
              <p class="text-sm font-semibold text-white">360p (Transcoded)</p>
              <p class="text-xs text-gray-400">Tiny file size (~1.0 Mbps, ideal for low storage)</p>
            </div>
          </label>
        </div>
      </div>
    </div>

    <!-- Modal Footer -->
    <div class="px-5 py-4 border-t border-white/10 flex items-center justify-end gap-3 bg-[rgba(6,10,18,0.4)]">
      <button
        onclick={onClose}
        class="px-4 py-2 text-xs font-semibold border border-white/15 rounded-xl text-gray-300 hover:text-white hover:bg-white/5 transition-colors"
      >
        Cancel
      </button>
      <button
        onclick={() => onConfirm(downloadQualitySelection)}
        class="px-4 py-2 text-xs font-semibold rounded-xl text-slate-950 bg-gradient-to-br from-cyan-200 via-sky-300 to-amber-300 hover:brightness-110 shadow-lg transition-all"
      >
        Start Download
      </button>
    </div>
  </div>
</div>
