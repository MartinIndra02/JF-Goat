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

<main class="app-stage min-h-screen text-[var(--text-primary)] flex items-center justify-center px-4 py-10">
  <div class="w-full max-w-md glass-panel-strong app-soft-ring rounded-3xl p-7 sm:p-9 flex flex-col items-center gap-6 app-animate-fade-up">
    <div class="text-center space-y-2">
      <p class="inline-flex items-center px-3 py-1 app-pill text-[11px] uppercase tracking-[0.18em]">Server Setup</p>
      <h1 class="text-3xl font-semibold">JF Goat</h1>
      <p class="app-muted text-sm">Connect to your Jellyfin server</p>
    </div>

    <form class="w-full flex flex-col gap-4" onsubmit={(e) => { e.preventDefault(); handleConnect(); }}>
      <TextInput
        bind:value={url}
        placeholder="http://your-server:8096"
        disabled={loading}
      />

      <Button type="submit" size="lg" disabled={loading || !url.trim()}>
        {loading ? "Connecting..." : "Connect"}
      </Button>
    </form>
  </div>
</main>
