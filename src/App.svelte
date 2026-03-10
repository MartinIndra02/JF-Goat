<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let name = $state("");
  let greeting = $state("");

  async function greet() {
    greeting = await invoke("greet", { name });
  }
</script>

<main class="min-h-screen bg-gray-900 text-white flex flex-col items-center justify-center gap-6">
  <div class="text-center">
    <h1 class="text-4xl font-bold mb-2">jfgoat</h1>
    <p class="text-gray-400 text-sm">Jellyfin Client &mdash; Scaffolding Complete</p>
  </div>

  <form class="flex gap-2" onsubmit={(e) => { e.preventDefault(); greet(); }}>
    <input
      class="px-4 py-2 rounded bg-gray-800 border border-gray-700 text-white placeholder-gray-500 focus:outline-none focus:border-blue-500"
      placeholder="Enter a name..."
      bind:value={name}
    />
    <button
      type="submit"
      class="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded font-medium transition-colors"
    >
      Greet
    </button>
  </form>

  {#if greeting}
    <p class="text-green-400 text-lg">{greeting}</p>
  {/if}
</main>
