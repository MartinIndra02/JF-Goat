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
      container: "bg-red-900/55 border-red-700 text-red-100",
      accent: "text-red-300",
    },
    warning: {
      container: "bg-yellow-900/55 border-yellow-700 text-yellow-100",
      accent: "text-yellow-300",
    },
    info: {
      container: "bg-blue-900/55 border-blue-700 text-blue-100",
      accent: "text-blue-300",
    },
    success: {
      container: "bg-green-900/55 border-green-700 text-green-100",
      accent: "text-green-300",
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
        ? "px-3 py-2.5 shadow-xl backdrop-blur-sm"
        : "px-4 py-3"
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
          class="shrink-0 text-xs px-2 py-1 rounded border border-white/20 hover:bg-white/10 transition-colors"
          onclick={handleDismiss}
          aria-label="Dismiss notification"
        >
          Close
        </button>
      {/if}
    </div>
  </div>
{/if}
