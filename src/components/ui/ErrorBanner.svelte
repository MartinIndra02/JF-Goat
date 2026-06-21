<script lang="ts">
  export type BannerTone = "error" | "warning" | "info" | "success";
  export type BannerVariant = "inline" | "toast";

  interface ErrorBannerProps {
    message: string;
    title?: string;
    tone?: BannerTone;
    variant?: BannerVariant;
    dismissible?: boolean;
    onDismiss?: (() => void) | undefined;
  }

  let {
    message,
    title = "",
    tone = "error",
    variant = "inline",
    dismissible = false,
    onDismiss,
  }: ErrorBannerProps = $props();

  const toneClasses: Record<BannerTone, { container: string; accent: string }> = {
    error: {
      container:
        "border-red-300/30 bg-[linear-gradient(140deg,rgba(239,68,68,0.34),rgba(127,29,29,0.22))] text-red-50",
      accent: "text-red-100",
    },
    warning: {
      container:
        "border-amber-300/30 bg-[linear-gradient(140deg,rgba(245,158,11,0.3),rgba(146,64,14,0.2))] text-amber-50",
      accent: "text-amber-100",
    },
    info: {
      container:
        "border-cyan-300/30 bg-[linear-gradient(140deg,rgba(6,182,212,0.28),rgba(8,47,73,0.2))] text-cyan-50",
      accent: "text-cyan-100",
    },
    success: {
      container:
        "border-emerald-300/30 bg-[linear-gradient(140deg,rgba(16,185,129,0.3),rgba(6,78,59,0.2))] text-emerald-50",
      accent: "text-emerald-100",
    },
  };

  const isToast = $derived(variant === "toast");
  const classes = $derived(toneClasses[tone]);

  function handleDismiss() {
    onDismiss?.();
  }
</script>

{#if message}
  <div
    class={`w-full border rounded ${classes.container} ${
      isToast
        ? "rounded-xl px-3 py-2.5 shadow-[0_16px_38px_rgba(3,10,23,0.44)] backdrop-blur-lg"
        : "rounded-xl px-4 py-3 backdrop-blur-sm"
    }`}
    role="alert"
    aria-live="assertive"
  >
    <div class="flex items-start gap-3">
      <div class="flex-1 min-w-0">
        {#if title}
          <p class="text-sm font-semibold {classes.accent}">{title}</p>
        {/if}
        <p class="text-sm break-words">{message}</p>
      </div>

      {#if dismissible}
        <button
          type="button"
          class="shrink-0 text-xs px-2 py-1 rounded-lg border border-white/25 hover:bg-white/12 transition-colors"
          onclick={handleDismiss}
          aria-label="Dismiss notification"
        >
          Close
        </button>
      {/if}
    </div>
  </div>
{/if}
