<script lang="ts">
  import { connectToServer } from "../lib/api";
  import { pushErrorToast, normalizeErrorMessage } from "../lib/stores/toast.svelte";
  import TextInput from "../components/ui/TextInput.svelte";
  import Button from "../components/ui/Button.svelte";
  import { push } from "svelte-spa-router";

  let url = $state("");
  let loading = $state(false);

  async function handleConnect() {
    loading = true;
    try {
      await connectToServer(url);
      push("/login");
    } catch (e: any) {
      pushErrorToast(
        "api",
        normalizeErrorMessage(e),
        "Connection failed",
        "connect-failed",
      );
    } finally {
      loading = false;
    }
  }
</script>

<main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
  <div class="w-full max-w-sm flex flex-col items-center gap-6 p-8">
    <div class="text-center">
      <h1 class="text-3xl font-bold mb-1">jfgoat</h1>
      <p class="text-gray-400 text-sm">Connect to your Jellyfin server</p>
    </div>

    <form class="w-full flex flex-col gap-4" onsubmit={(e) => { e.preventDefault(); handleConnect(); }}>
      <TextInput
        bind:value={url}
        placeholder="http://your-server:8096"
        disabled={loading}
      />

      <Button type="submit" disabled={loading || !url.trim()}>
        {loading ? "Connecting..." : "Connect"}
      </Button>
    </form>
  </div>
</main>
