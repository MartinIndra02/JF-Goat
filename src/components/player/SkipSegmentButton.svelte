<script lang="ts">
  let {
    label,
    onSkip,
  }: {
    label: string;
    onSkip: () => void;
  } = $props();
</script>

<button
  onclick={onSkip}
  type="button"
  class="pointer-events-auto px-5 py-3 inline-flex items-center gap-2 rounded-xl bg-[rgba(15,23,42,0.65)] hover:bg-[rgba(15,23,42,0.85)] border border-white/20 hover:border-cyan-400/40 text-white font-medium text-sm transition-all duration-300 backdrop-blur-2xl shadow-[0_12px_40px_rgba(0,0,0,0.6)] cursor-pointer select-none active:scale-[0.98] group hover:shadow-[0_12px_40px_rgba(34,211,238,0.2)] animate-slide-in"
>
  <!-- Fast Forward icon -->
  <svg class="w-4 h-4 text-cyan-300 group-hover:translate-x-0.5 transition-transform" fill="none" stroke="currentColor" stroke-width="2.5" viewBox="0 0 24 24">
    <path stroke-linecap="round" stroke-linejoin="round" d="M3 5v14l8-7zm9 0v14l8-7z" />
  </svg>
  <span>{label}</span>
  <kbd class="ml-1.5 px-1.5 py-0.5 text-[10px] font-semibold text-slate-400 bg-white/10 rounded border border-white/10">S</kbd>
</button>

<style>
  @keyframes slideIn {
    from {
      opacity: 0;
      transform: translateX(20px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  .animate-slide-in {
    animation: slideIn 0.35s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }
</style>
