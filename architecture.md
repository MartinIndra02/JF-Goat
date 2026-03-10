# Architectural Design: Ultra-Fast Jellyfin Client for Windows

## Overview

This document outlines the architecture for a highly optimized, native-feeling desktop client for Jellyfin on Windows. It is specifically designed to handle massive libraries (100,000+ items) with sub-50ms load times and zero UI blocking.

By abandoning the traditional heavy web-client approach (Electron, DOM-heavy rendering, synchronous API fetching) and adopting an **Offline-First** mindset combined with **Native Video Rendering**, this architecture ensures extreme performance and a premium user experience.

---

## 1. Technology Stack

The application is split into a high-performance native backend and a lightweight, compiled frontend.

| Layer | Technology | Purpose & Justification |
| --- | --- | --- |
| **Core / Backend** | Tauri (Rust) | Replaces Electron. Provides OS-level access, handles background tasks, bypasses browser CORS limitations, and compiles into a tiny `.exe`/`.msi`. |
| **Frontend / UI** | Svelte + Vite | Compiler-based framework (no Virtual DOM). Offers absolute control over UI, lightning-fast list rendering, and built-in transition animations. |
| **Video Player** | `libmpv` (C/Rust) | Industry-standard for hardware-accelerated playback (DXVA2/D3D11VA). Highly customizable and bypasses webview codec limitations. |
| **Local State** | SQLite (via Rust) | Offline-first database acting as the primary source of truth for the UI. Ensures instant startup. |
| **Styling** | Tailwind CSS | Utility-first CSS framework for rapid development of a custom, premium UI (e.g., Netflix/Spotify-like design). |

---

## 2. Synchronization Strategy (The "Two-Track" System)

To make the app feel instantaneous, we separate the data loading into two distinct tracks: **UI Priority Fetch** and **Background Delta Sync**.

### Track A: Fast Track (UI Priority)

This runs the moment the user opens the app. The goal is to get actionable items on the screen immediately.

1. **Next Up & Continue Playing:** The app calls Jellyfin API endpoints (`/Users/{userId}/Items/Resume` and `/Shows/NextUp`).
* *Optimization:* We request **only IDs and progress ticks** (empty `Fields` parameter).
* Rust takes these IDs, instantly pulls the corresponding titles and local image paths from the local SQLite database, and Svelte renders them.


2. **Recently Added:** The same lightweight ID-fetch is applied to recently added Movies, Shows, and Anime.

### Track B: Slow Track (Background Maintenance)

While the user is already interacting with the loaded UI, Rust silently performs a deep sync to keep the local 100k+ SQLite database accurate.

1. **Metadata Updates:** Rust queries the API for items where **`DateUpdated`** (not `DateCreated`) is newer than the last app launch. This catches changed posters, updated descriptions, or modified watch statuses on older movies.
2. **Ghost Cleanup (Deletions):** Rust compares the total server item count with the local database count. If the server has fewer items, a background diff is run to remove deleted items from the local SQLite to prevent dead links.
3. **Real-time Updates:** A persistent WebSocket connection listens for `UserDataChanged` events to instantly reflect changes made on other devices (e.g., watching an episode on a phone).

---

## 3. Player Integration: The Transparent Underlay

Rendering a native C-based video player (`libmpv`) inside a webview is a major bottleneck. To enable seamless transitions, such as a "Mini-player" or audio bar while browsing, we use a layered underlay approach.

### Architecture Flow:

1. **The Native Layer (Bottom):** Rust spawns a hidden, borderless, native Windows window (HWND). `libmpv` renders hardware-accelerated video directly into this window.
2. **The Web Layer (Top):** The Tauri application window (hosting Svelte) is configured to be **transparent**. It sits exactly on top of the native video window.
3. **The "Hole Punch":** When a video plays, Svelte renders a completely transparent `<div>` covering the viewport. The user sees the native video *through* the web frontend, while Svelte draws UI controls (play, pause, timeline) on top.
4. **Mini-Player Transition:** If the user minimizes the player to browse the library, Svelte sends the new desired coordinates via IPC to Rust. Rust instantly resizes and moves the underlying `mpv` window, while Svelte animates the UI to match it.

---

## 4. Aggressive Image Caching (LRU)

Downloading 110,000 posters via HTTP would destroy network bandwidth and UI performance.

* **Optimized Fetching:** Images are never fetched at original size. Rust requests them via API with `?maxWidth=400&format=webp` to compress them to ~15-30 KB.
* **Local Storage:** Images are saved directly to the local disk (e.g., `%APPDATA%\YourApp\cache`).
* **Custom Protocol:** Svelte loads images using a custom Tauri scheme (`asset://`), bypassing standard HTTP overhead and ensuring zero-lag scrolling.
* **LRU (Least Recently Used) Cleanup:** Rust enforces a strict cache size limit (e.g., 2 GB). Once exceeded, a background worker automatically deletes the image files of the least recently viewed movies.

---

## 5. Search Optimization & Security

* **Sub-Millisecond Search:** Standard SQL `LIKE` queries are too slow for "search-as-you-type" across 110k items. The SQLite database uses the **FTS5 (Full-Text Search)** extension. This creates a virtual table index allowing instant, typo-tolerant (fuzzy) searching directly from the local disk.
* **Token Security:** The Jellyfin API `AccessToken` is **never** stored in the browser's `localStorage`. It is securely encrypted and stored in the Windows Credential Manager using the `tauri-plugin-stronghold` library. Svelte only triggers requests; Rust attaches the secret token backend-side before dispatching the HTTP call.
