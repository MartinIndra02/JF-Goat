<script lang="ts">
  import { onMount } from "svelte";
  import Router from "svelte-spa-router";
  import { replace } from "svelte-spa-router";
  import { checkAuth, checkAuthOffline } from "./lib/api";
  import {
    getAuthStatus,
    setAuthenticated,
    setUnauthenticated,
  } from "./lib/stores/auth.svelte";
  import {
    initPlayerListeners,
    isPlayerVisible,
    hidePlayer,
  } from "./lib/stores/player.svelte";
  import {
    getPreferences,
    loadPreferences,
  } from "./lib/stores/preferences.svelte";
  import {
    setOnlineStatus,
    markDegraded,
    markHealthy,
  } from "./lib/stores/connectivity.svelte";
  import {
    getToasts,
    dismissToast,
    pushToast,
    pushErrorToast,
    normalizeErrorMessage,
    type ToastSource,
  } from "./lib/stores/toast.svelte";
  import LoadingScreen from "./components/layout/LoadingScreen.svelte";
  import ErrorBanner from "./components/ui/ErrorBanner.svelte";
  import Player from "./components/player/Player.svelte";
  import ServerConnect from "./views/ServerConnect.svelte";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";
  import ItemDetail from "./views/ItemDetail.svelte";

  const routes = {
    "/": Home,
    "/connect": ServerConnect,
    "/login": Login,
    "/home": Home,
    "/library": Home,
    "/search": Home,
    "/settings": Home,
    "/item": ItemDetail,
    "/item/:id": ItemDetail,
    "*": Home,
  };

  const playerActive = $derived(isPlayerVisible());
  const toasts = $derived(getToasts());

  function applyPreferencesToLocalPlayerKeys() {
    if (typeof localStorage === "undefined") return;

    const prefs = getPreferences();
    localStorage.setItem(
      "jfgoat.player.defaultPlaybackRate",
      String(prefs.playback.default_playback_rate),
    );
    localStorage.setItem(
      "jfgoat.player.defaultQualityKey",
      prefs.quality.default_quality_key,
    );

    if (prefs.language.preferred_audio_language) {
      localStorage.setItem(
        "jfgoat.player.preferredAudioLanguage",
        prefs.language.preferred_audio_language,
      );
    }

    if (prefs.language.preferred_subtitle_language) {
      localStorage.setItem(
        "jfgoat.player.preferredSubtitleLanguage",
        prefs.language.preferred_subtitle_language,
      );
    }
  }

  function getRequestedPathAndQuery(): string {
    const hash = window.location.hash || "";
    if (!hash.startsWith("#/")) {
      return "/home";
    }

    const parsed = hash.slice(1);
    return parsed.length > 0 ? parsed : "/home";
  }

  function isGuestRoute(path: string): boolean {
    return path.startsWith("/connect") || path.startsWith("/login");
  }

  function isAuthedRoute(path: string): boolean {
    return (
      path.startsWith("/home") ||
      path.startsWith("/library") ||
      path.startsWith("/search") ||
      path.startsWith("/settings") ||
      path.startsWith("/item")
    );
  }

  function sourceFromErrorMessage(message: string): ToastSource {
    const lower = message.toLowerCase();
    if (lower.includes("mpv") || lower.includes("playback")) {
      return "player";
    }
    return "api";
  }

  function isIgnorableRuntimeError(message: string): boolean {
    const lower = message.toLowerCase();
    return lower.includes("resizeobserver loop") || lower.includes("script error");
  }

  async function init() {
    const requestedPath = getRequestedPathAndQuery();

    try {
      await loadPreferences();
      applyPreferencesToLocalPlayerKeys();

      // Phase 1: Fast offline check — shows homepage instantly from cached data
      const offlineSession = await checkAuthOffline();
      if (offlineSession) {
        setAuthenticated(offlineSession);
        if (isGuestRoute(requestedPath) || !isAuthedRoute(requestedPath)) {
          replace("/home");
        }

        // Phase 2: Verify token in the background; auto-login if expired
        checkAuth()
          .then((session) => {
            if (!session) {
              // Token expired and auto-login failed — redirect to login
              pushToast({
                level: "warning",
                source: "api",
                title: "Session expired",
                message: "Please sign in again.",
                dedupeKey: "session-expired",
              });
              setUnauthenticated();
              replace("/login");
            } else {
              // Update session (may have been refreshed via auto-login)
              setAuthenticated(session);
              markHealthy();
            }
          })
          .catch((error) => {
            // Network error — stay on homepage with cached data
            markDegraded(normalizeErrorMessage(error));
            pushToast({
              level: "info",
              source: "api",
              title: "Offline mode",
              message: "Server verification failed. Using cached session.",
              dedupeKey: `offline-session-${normalizeErrorMessage(error)}`,
              dismissAfterMs: 4500,
            });
          });
        return;
      }

      // No cached token — attempt auto-login (network required) before connect
      const session = await checkAuth().catch(() => null);
      if (session) {
        setAuthenticated(session);
        markHealthy();
        if (isGuestRoute(requestedPath) || !isAuthedRoute(requestedPath)) {
          replace("/home");
        }
        return;
      }

      setUnauthenticated();
      if (!isGuestRoute(requestedPath)) {
        replace("/connect");
      }
    } catch (error) {
      markDegraded(normalizeErrorMessage(error));
      pushErrorToast(
        "api",
        normalizeErrorMessage(error),
        "Startup failed",
        "startup-init-failed",
      );
      setUnauthenticated();
      replace("/connect");
    }
  }

  onMount(() => {
    // Ensure stale dev/HMR player state never hides the whole app shell on boot.
    hidePlayer();

    try {
      initPlayerListeners();
    } catch (error) {
      pushErrorToast(
        "player",
        normalizeErrorMessage(error),
        "Player initialization failed",
        "player-listener-init-failed",
      );
    }

    const initialOnline = typeof navigator === "undefined" ? true : navigator.onLine;
    setOnlineStatus(initialOnline);
    if (initialOnline) {
      markHealthy();
    }

    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      const message = normalizeErrorMessage(event.reason);
      if (isIgnorableRuntimeError(message)) return;

      const source = sourceFromErrorMessage(message);
      const title = source === "player" ? "Playback error" : "Request failed";
      pushErrorToast(source, message, title, `unhandled-rejection-${source}-${message}`);
    };

    const handleRuntimeError = (event: ErrorEvent) => {
      const message = normalizeErrorMessage(event.error ?? event.message);
      if (isIgnorableRuntimeError(message)) return;

      const source = sourceFromErrorMessage(message);
      const title = source === "player" ? "Playback error" : "Unexpected error";
      pushErrorToast(source, message, title, `runtime-error-${source}-${message}`);
    };

    const handleOnline = () => {
      setOnlineStatus(true);
      markHealthy();
      pushToast({
        level: "success",
        source: "system",
        title: "Back online",
        message: "Connection restored.",
        dedupeKey: "online-restored",
        dismissAfterMs: 3000,
      });
    };

    const handleOffline = () => {
      setOnlineStatus(false);
      markDegraded("Network connection unavailable");
      pushToast({
        level: "warning",
        source: "system",
        title: "Offline",
        message: "You are offline. Showing cached data when available.",
        dedupeKey: "offline-detected",
      });
    };

    window.addEventListener("unhandledrejection", handleUnhandledRejection);
    window.addEventListener("error", handleRuntimeError);
    window.addEventListener("online", handleOnline);
    window.addEventListener("offline", handleOffline);

    void init();

    return () => {
      window.removeEventListener("unhandledrejection", handleUnhandledRejection);
      window.removeEventListener("error", handleRuntimeError);
      window.removeEventListener("online", handleOnline);
      window.removeEventListener("offline", handleOffline);
    };
  });
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

{#if toasts.length > 0}
  <div class="fixed top-4 right-4 z-[100] w-[min(92vw,24rem)] space-y-2 pointer-events-none">
    {#each toasts as toast (toast.id)}
      <div class="pointer-events-auto">
        <ErrorBanner
          message={toast.message}
          title={toast.title}
          tone={toast.level}
          variant="toast"
          dismissible={true}
          onDismiss={() => dismissToast(toast.id)}
        />
      </div>
    {/each}
  </div>
{/if}

<Player />
