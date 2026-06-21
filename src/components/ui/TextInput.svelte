<script lang="ts">
  let {
    value = $bindable(""),
    element = $bindable<HTMLInputElement | null>(null),
    placeholder = "",
    type = "text",
    disabled = false,
    oninput,
    onkeydown,
    inputClass = "",
  }: {
    value: string;
    element?: HTMLInputElement | null;
    placeholder?: string;
    type?: string;
    disabled?: boolean;
    oninput?: (e: Event) => void;
    onkeydown?: (e: KeyboardEvent) => void;
    inputClass?: string;
  } = $props();
</script>

<input
  bind:this={element}
  {type}
  {placeholder}
  {disabled}
  bind:value
  {oninput}
  {onkeydown}
  class={`w-full h-10 rounded-xl border border-[rgba(156,176,214,0.24)] bg-[rgba(13,21,35,0.72)] px-4 text-sm text-[var(--text-primary)] placeholder:text-[var(--text-tertiary)] shadow-[inset_0_1px_0_rgba(255,255,255,0.08)] backdrop-blur-md transition-all duration-200 focus:outline-none focus:border-[rgba(102,216,255,0.45)] focus:bg-[rgba(19,30,49,0.76)] focus:shadow-[0_0_0_2px_rgba(102,216,255,0.17)] disabled:opacity-50 disabled:cursor-not-allowed ${inputClass}`}
/>
