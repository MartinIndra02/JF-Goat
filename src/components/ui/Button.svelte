<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    onclick,
    type = "button",
    disabled = false,
    variant = "primary",
    size = "md",
    className = "",
    children,
  }: {
    onclick?: () => void;
    type?: "button" | "submit";
    disabled?: boolean;
    variant?: "primary" | "secondary" | "danger";
    size?: "sm" | "md" | "lg";
    className?: string;
    children: Snippet;
  } = $props();

  const baseClass =
    "inline-flex items-center justify-center gap-2 rounded-xl font-semibold tracking-[0.01em] transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent-2)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--bg-1)]";

  const sizeClasses: Record<string, string> = {
    sm: "h-9 px-3 text-xs",
    md: "h-10 px-4 text-sm",
    lg: "h-11 px-5 text-sm",
  };

  const variants: Record<string, string> = {
    primary:
      "text-slate-950 bg-[linear-gradient(135deg,#66d8ff_0%,#41b8d5_54%,#f4bc6b_100%)] shadow-[0_10px_26px_rgba(65,184,213,0.42)] hover:-translate-y-0.5 hover:brightness-110",
    secondary:
      "text-slate-100 border border-[rgba(164,182,216,0.28)] bg-[rgba(17,26,41,0.7)] backdrop-blur-md hover:bg-[rgba(42,57,84,0.72)]",
    danger:
      "text-red-50 border border-red-300/30 bg-red-500/25 hover:bg-red-500/35",
  };
</script>

<button
  {type}
  {disabled}
  {onclick}
  class="{baseClass} {sizeClasses[size] ?? sizeClasses.md} {variants[variant] ?? variants.primary} {className}"
>
  {@render children()}
</button>
