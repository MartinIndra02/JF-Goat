<script lang="ts">
  let {
    autoplayCountdown,
    cancelAutoplayCountdown,
  }: {
    autoplayCountdown: number;
    cancelAutoplayCountdown: () => void;
  } = $props();
</script>

<button
  onclick={cancelAutoplayCountdown}
  class="h-8 px-3 inline-flex items-center rounded-lg bg-amber-400/16 border border-amber-200/35 text-amber-100 text-xs font-medium hover:bg-amber-400/26 transition-colors"
>
  Cancel Next Up ({autoplayCountdown}s)
</button>
