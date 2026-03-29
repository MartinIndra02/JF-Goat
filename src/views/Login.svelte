<script lang="ts">
  import { login as loginApi } from "../lib/api";
  import { setAuthenticated } from "../lib/stores/auth.svelte";
  import { pushErrorToast, normalizeErrorMessage } from "../lib/stores/toast.svelte";
  import TextInput from "../components/ui/TextInput.svelte";
  import Button from "../components/ui/Button.svelte";
  import { push } from "svelte-spa-router";

  let username = $state("");
  let password = $state("");
  let loading = $state(false);

  async function handleLogin() {
    loading = true;
    try {
      const result = await loginApi(username, password);
      setAuthenticated({
        user_id: result.user_id,
        username: result.username,
        server_id: result.server_id,
        server_name: "",
        server_url: "",
      });
      push("/home");
    } catch (e: any) {
      pushErrorToast(
        "api",
        normalizeErrorMessage(e),
        "Sign-in failed",
        "login-failed",
      );
    } finally {
      loading = false;
    }
  }
</script>

<main class="min-h-screen bg-gray-900 text-white flex items-center justify-center">
  <div class="w-full max-w-sm flex flex-col items-center gap-6 p-8">
    <div class="text-center">
      <h1 class="text-3xl font-bold mb-1">Sign In</h1>
      <p class="text-gray-400 text-sm">Enter your Jellyfin credentials</p>
    </div>

    <form class="w-full flex flex-col gap-4" onsubmit={(e) => { e.preventDefault(); handleLogin(); }}>
      <TextInput
        bind:value={username}
        placeholder="Username"
        disabled={loading}
      />
      <TextInput
        bind:value={password}
        placeholder="Password"
        type="password"
        disabled={loading}
      />

      <Button type="submit" disabled={loading || !username.trim()}>
        {loading ? "Signing in..." : "Sign In"}
      </Button>
    </form>

    <button
      class="text-gray-500 text-sm hover:text-gray-300 transition-colors"
      onclick={() => push("/connect")}
    >
      &larr; Different server
    </button>
  </div>
</main>
