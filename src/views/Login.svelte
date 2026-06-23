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
        server_name: result.server_name,
        server_url: result.server_url,
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

<main class="app-stage min-h-screen text-[var(--text-primary)] flex items-center justify-center px-4 py-10">
  <div class="w-full max-w-md glass-panel-strong app-soft-ring rounded-3xl p-7 sm:p-9 flex flex-col items-center gap-6 app-animate-fade-up">
    <div class="text-center space-y-2">
      <p class="inline-flex items-center px-3 py-1 app-pill text-[11px] uppercase tracking-[0.18em]">Account Access</p>
      <h1 class="text-3xl font-semibold">Sign In</h1>
      <p class="app-muted text-sm">Enter your Jellyfin credentials</p>
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

      <Button type="submit" size="lg" disabled={loading || !username.trim()}>
        {loading ? "Signing in..." : "Sign In"}
      </Button>
    </form>

    <button
      class="text-sm app-faint hover:text-[var(--text-secondary)] transition-colors"
      onclick={() => push("/connect")}
    >
      &larr; Different server
    </button>
  </div>
</main>
