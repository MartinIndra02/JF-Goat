<script lang="ts">
  let {
    value = $bindable(""),
    placeholder = "",
    type = "text",
    disabled = false,
  }: {
    value: string;
    placeholder?: string;
    type?: string;
    disabled?: boolean;
  } = $props();
</script>

<input
  {type}
  {placeholder}
  {disabled}
  bind:value
  class="w-full px-4 py-2 rounded bg-gray-800 border border-gray-700 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500 disabled:opacity-50"
/>
