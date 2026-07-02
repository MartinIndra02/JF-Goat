<script lang="ts">
  let {
    effectivePos,
    dur,
    progressPercent,
    chapterMarkers,
    mediaSegments = [],
    isScrubbing,
    progressScrubEl = $bindable(null),
    beginTimelineScrub,
    handleProgressKeydown,
    seekToChapter,
  }: {
    effectivePos: number;
    dur: number;
    progressPercent: number;
    chapterMarkers: Array<{ percent: number, startSeconds: number, name: string }>;
    mediaSegments?: Array<{ start_ticks: number, end_ticks: number, segment_type: string }>;
    isScrubbing: boolean;
    progressScrubEl: HTMLElement | null;
    beginTimelineScrub: (e: PointerEvent) => void;
    handleProgressKeydown: (e: KeyboardEvent) => void;
    seekToChapter: (seconds: number) => void;
  } = $props();

  const segmentMarkers = $derived.by(() => {
    if (dur <= 0 || !mediaSegments) return [];
    return mediaSegments.map((seg) => {
      const startSec = seg.start_ticks / 10_000_000;
      const endSec = seg.end_ticks / 10_000_000;
      const leftPercent = Math.max(0, Math.min((startSec / dur) * 100, 100));
      const rightPercent = Math.max(0, Math.min((endSec / dur) * 100, 100));
      const widthPercent = Math.max(0, rightPercent - leftPercent);
      
      const type = (seg.segment_type ?? "").toLowerCase();
      let colorClass = "bg-amber-400/40 border-l border-r border-amber-300/40";
      if (type === "outro") {
        colorClass = "bg-purple-500/40 border-l border-r border-purple-400/40";
      } else if (type === "recap") {
        colorClass = "bg-cyan-400/40 border-l border-r border-cyan-300/40";
      }

      return {
        left: leftPercent,
        width: widthPercent,
        label: seg.segment_type,
        colorClass,
      };
    });
  });
</script>

<div
  bind:this={progressScrubEl}
  class="group w-full h-5 sm:h-6 flex items-center mb-2 cursor-grab"
  class:cursor-grabbing={isScrubbing}
  onpointerdown={beginTimelineScrub}
  onkeydown={handleProgressKeydown}
  role="slider"
  aria-label="Seek in video"
  aria-valuenow={Math.floor(effectivePos)}
  aria-valuemin={0}
  aria-valuemax={Math.floor(dur)}
  tabindex="0"
>
  <div class="w-full h-2 bg-white/16 rounded-full transition-all relative overflow-visible group-hover:shadow-[0_0_0_1px_rgba(102,216,255,0.35)]">
    <div
      class="absolute top-0 left-0 h-full bg-gradient-to-r from-cyan-300 via-sky-400 to-amber-300 rounded-full"
      style="width: {progressPercent}%"
    ></div>

    <!-- Segment markers rendered on top of progress bar with pointer-events-none -->
    {#each segmentMarkers as seg}
      <div
        class="absolute top-0 h-full {seg.colorClass} transition-colors rounded-sm pointer-events-none"
        style="left: {seg.left}%; width: {seg.width}%"
        title={seg.label}
      ></div>
    {/each}

    <div
      class="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 h-4 w-4 rounded-full bg-cyan-200 border border-white/75 shadow-[0_0_0_2px_rgba(0,0,0,0.35)] transition-transform"
      class:scale-125={isScrubbing}
      style="left: {progressPercent}%"
    ></div>

    {#each chapterMarkers as chapter}
      <button
        type="button"
        class="absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-2 h-4 rounded bg-amber-100/80 hover:bg-amber-200 transition-colors"
        style="left: {chapter.percent}%"
        onpointerdown={(e) => e.stopPropagation()}
        onclick={(e) => {
          e.stopPropagation();
          void seekToChapter(chapter.startSeconds);
        }}
        aria-label={`Jump to chapter: ${chapter.name}`}
        title={chapter.name}
      ></button>
    {/each}
  </div>
</div>
