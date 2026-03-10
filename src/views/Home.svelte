<script lang="ts">
  import { logout as logoutApi } from "../lib/api";
  import { getSession, setUnauthenticated } from "../lib/stores/auth.svelte";
  import Button from "../components/ui/Button.svelte";
  import { push } from "svelte-spa-router";

  const session = getSession();

  async function handleLogout() {
    try {
      await logoutApi();
    } catch {
      // Best effort
    }
    setUnauthenticated();
    push("/connect");
  }
</script>

<main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
  <div class="flex flex-col items-center gap-6 p-8">
    <h1 class="text-3xl font-bold">
      Welcome{session?.username ? `, ${session.username}` : ""}
    </h1>
    <p class="text-gray-400 text-sm">
      {#if session?.server_name}
        Connected to {session.server_name}
      {:else}
        Connected to Jellyfin
      {/if}
    </p>

    <Button variant="danger" onclick={handleLogout}>
      Log Out
    </Button>
  </div>
</main>
