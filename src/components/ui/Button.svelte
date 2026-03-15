<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    onclick,
    type = "button",
    disabled = false,
    variant = "primary",
    children,
  }: {
    onclick?: () => void;
    type?: "button" | "submit";
    disabled?: boolean;
    variant?: "primary" | "secondary" | "danger";
    children: Snippet;
  } = $props();

  const variants: Record<string, string> = {
    primary: "bg-blue-600 hover:bg-blue-700",
    secondary: "bg-gray-700 hover:bg-gray-600",
    danger: "bg-red-600 hover:bg-red-700",
  };
</script>

<button
  {type}
  {disabled}
  {onclick}
  class="px-4 py-2 rounded font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed {variants[variant]}"
>
  {@render children()}
</button>
