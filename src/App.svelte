<script lang="ts">
  import Router from "svelte-spa-router";
  import { push } from "svelte-spa-router";
  import { checkAuth, checkAuthOffline } from "./lib/api";
  import {
    getAuthStatus,
    setAuthenticated,
    setUnauthenticated,
  } from "./lib/stores/auth.svelte";
  import LoadingScreen from "./components/layout/LoadingScreen.svelte";
  import ServerConnect from "./views/ServerConnect.svelte";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";

  const routes = {
    "/connect": ServerConnect,
    "/login": Login,
    "/home": Home,
  };

  async function init() {
    try {
      // Phase 1: Fast offline check — shows homepage instantly from cached data
      const offlineSession = await checkAuthOffline();
      if (offlineSession) {
        setAuthenticated(offlineSession);
        push("/home");

        // Phase 2: Verify token in the background (no spinner shown)
        checkAuth().then((session) => {
          if (!session) {
            // Token expired — redirect to connect
            setUnauthenticated();
            push("/connect");
          }
        }).catch(() => {
          // Network error — stay on homepage with cached data
        });
        return;
      }

      // No cached session — fall back to full auth check
      setUnauthenticated();
      push("/connect");
    } catch {
      setUnauthenticated();
      push("/connect");
    }
  }

  init();
</script>

{#if getAuthStatus() === "loading"}
  <LoadingScreen />
{:else}
  <Router {routes} />
{/if}
