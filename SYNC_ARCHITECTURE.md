# Feature Spec: Initial Sync, Rate Limiting & Search Fallback

## 1. Context & Goal

The application ("jfgoat") is an offline-first Jellyfin desktop client built with Tauri (Rust) and Svelte. It is designed to handle massive libraries (100,000+ items).
To prevent long loading screens on the very first startup (when the local SQLite database is empty), the application must allow the user to immediately browse the "Fast Track" UI (Next Up, Continue Watching) while silently indexing the rest of the 100k+ library in the background.

## 2. Application State Management

The Rust backend must maintain a global state (e.g., using `std::sync::Mutex` or `tokio::sync::RwLock` wrapped in Tauri's managed state) to track the synchronization status.

**States:**

* `INITIAL_SYNC`: The application is currently downloading the 100k+ library and building the local SQLite database.
* `READY`: The local database is fully populated and up-to-date.

## 3. Background Indexing Worker (Rust / Tokio)

The initial synchronization must run in a non-blocking background task to ensure the UI and video playback (`libmpv`) remain perfectly smooth.

**Implementation Requirements for the AI Agent:**

1. **Spawn Async Task:** Use `tokio::spawn` to run the indexing loop outside the main Tauri thread.
2. **Fetch Total Count:** First, query the Jellyfin API to get the `TotalRecordCount` of the user's library.
3. **Pagination:** Do not fetch 100,000 items at once. Loop through the `/Users/{userId}/Items` endpoint using `StartIndex` and `Limit` (e.g., chunks of 1000 items).
4. **Database Ingestion:** Insert the fetched chunk into the local SQLite database using a single SQL transaction per chunk to maximize disk I/O performance.
5. **Rate Limiting (Crucial):** After processing each chunk, the worker must explicitly yield/sleep (e.g., `tokio::time::sleep(Duration::from_millis(500))`). This prevents DDoS-ing the user's self-hosted Jellyfin server and ensures the server can still respond to the user's immediate playback requests.

## 4. Search Routing (Fallback Mechanism)

Because the local FTS5 (Full-Text Search) SQLite index is incomplete during `INITIAL_SYNC`, local searches would return missing data. The Tauri command handling search (`#[tauri::command] async fn search_items(...)`) must dynamically route the request based on the current state.

**Routing Logic:**

* `if state == INITIAL_SYNC`:
* Bypass SQLite entirely.
* Make an HTTP request directly to the remote Jellyfin API (`/Users/{userId}/Items?searchTerm={query}`).
* Return the remote JSON payload to the frontend.


* `if state == READY`:
* Query the local SQLite FTS5 virtual table.
* Return the local results (sub-millisecond response time).



## 5. Frontend Communication & Progress UI

The user must be aware that the library is optimizing in the background so they understand why a search might take a bit longer initially.

**Rust to Svelte Event Emission:**

* After every successfully ingested chunk, Rust must emit a Tauri window event (e.g., `"sync-progress"`).
* **Payload structure:** `{ "current": u32, "total": u32, "percentage": f32 }`
* When `current == total`, Rust updates the global state to `READY` and emits a final `"sync-complete"` event.

**Svelte Store Implementation:**

* The Svelte frontend must listen to `"sync-progress"` and update a reactive store.
* The UI should display a discrete, non-blocking indicator (e.g., a small spinner in the corner or bottom bar saying *"Indexing library... 45%"*).
* Once `"sync-complete"` is received, the indicator gracefully animates out (e.g., using Svelte's `fade` or `slide` transitions), and the UI relies entirely on the local SQLite cache from that point forward.
