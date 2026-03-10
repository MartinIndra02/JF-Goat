<script lang="ts">
  import Router from "svelte-spa-router";
  import { push } from "svelte-spa-router";
  import { checkAuth } from "./lib/api";
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
      const session = await checkAuth();
      if (session) {
        setAuthenticated(session);
        push("/home");
      } else {
        setUnauthenticated();
        push("/connect");
      }
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
