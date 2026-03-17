<script lang="ts">
  import Router from "svelte-spa-router";
  import { push } from "svelte-spa-router";
  import { checkAuth, checkAuthOffline } from "./lib/api";
  import {
    getAuthStatus,
    setAuthenticated,
    setUnauthenticated,
  } from "./lib/stores/auth.svelte";
  import {
    initPlayerListeners,
    isPlayerVisible,
  } from "./lib/stores/player.svelte";
  import LoadingScreen from "./components/layout/LoadingScreen.svelte";
  import Player from "./components/player/Player.svelte";
  import ServerConnect from "./views/ServerConnect.svelte";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";
  import ItemDetail from "./views/ItemDetail.svelte";

  const routes = {
    "/connect": ServerConnect,
    "/login": Login,
    "/home": Home,
    "/item": ItemDetail,
  };

  const playerActive = $derived(isPlayerVisible());

  initPlayerListeners();

  async function init() {
    try {
      // Phase 1: Fast offline check — shows homepage instantly from cached data
      const offlineSession = await checkAuthOffline();
      if (offlineSession) {
        setAuthenticated(offlineSession);
        push("/home");

        // Phase 2: Verify token in the background; auto-login if expired
        checkAuth()
          .then((session) => {
            if (!session) {
              // Token expired and auto-login failed — redirect to login
              setUnauthenticated();
              push("/login");
            } else {
              // Update session (may have been refreshed via auto-login)
              setAuthenticated(session);
            }
          })
          .catch(() => {
            // Network error — stay on homepage with cached data
          });
        return;
      }

      // No cached token — attempt auto-login (network required) before connect
      const session = await checkAuth().catch(() => null);
      if (session) {
        setAuthenticated(session);
        push("/home");
        return;
      }

      setUnauthenticated();
      push("/connect");
    } catch {
      setUnauthenticated();
      push("/connect");
    }
  }

  init();
</script>

<!-- Opaque app background (hidden when player is active to reveal mpv video) -->
<div class="fixed inset-0 bg-gray-900 -z-10" class:hidden={playerActive}></div>

{#if getAuthStatus() === "loading"}
  <LoadingScreen />
{:else}
  <div class:hidden={playerActive}>
    <Router {routes} />
  </div>
{/if}

<Player />
