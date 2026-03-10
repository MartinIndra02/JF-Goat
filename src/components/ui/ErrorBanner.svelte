<script lang="ts">
  let { message }: { message: string } = $props();
</script>

{#if message}
  <div class="w-full px-4 py-3 rounded bg-red-900/50 border border-red-700 text-red-300 text-sm">
    {message}
  </div>
{/if}
