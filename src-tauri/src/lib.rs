mod api;
mod commands;
mod db;
mod diagnostics;
mod error;
mod mpv;
mod state;
mod sync;

use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use std::sync::atomic::AtomicBool;
use serde::Serialize;
use tracing::{info, warn};

use state::{AppState, SyncStatus};

/// 1x1 transparent WebP (lossless) — returned on errors so <img> fails gracefully.
const TRANSPARENT_PIXEL_WEBP: &[u8] = &[
    0x52, 0x49, 0x46, 0x46, 0x24, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
    0x56, 0x50, 0x38, 0x4C, 0x17, 0x00, 0x00, 0x00, 0x2F, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

const IMAGE_CACHE_MAX_BYTES: u64 = 512 * 1024 * 1024;
const IMAGE_CACHE_MAX_FILES: usize = 3_000;
const IMAGE_CACHED_EVENT: &str = "jfimage-cached";

#[derive(Debug, Clone, Serialize)]
struct ImageCachedPayload {
    image_type: String,
    item_id: String,
    tag: String,
}

#[derive(Debug)]
struct CacheFileEntry {
    path: PathBuf,
    modified: SystemTime,
    size: u64,
}

/// Keep image cache bounded by file count and total size by deleting oldest files first.
fn cleanup_image_cache(cache_dir: &std::path::Path) {
    let read_dir = match fs::read_dir(cache_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    let mut entries: Vec<CacheFileEntry> = Vec::new();
    let mut total_bytes: u64 = 0;

    for dir_entry in read_dir.flatten() {
        let path = dir_entry.path();
        if !path.is_file() {
            continue;
        }

        let metadata = match dir_entry.metadata() {
            Ok(meta) if meta.is_file() => meta,
            _ => continue,
        };

        let size = metadata.len();
        total_bytes = total_bytes.saturating_add(size);

        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        entries.push(CacheFileEntry {
            path,
            modified,
            size,
        });
    }

    if entries.len() <= IMAGE_CACHE_MAX_FILES && total_bytes <= IMAGE_CACHE_MAX_BYTES {
        return;
    }

    // Oldest files are evicted first.
    entries.sort_by_key(|entry| entry.modified);

    let mut file_count = entries.len();
    let mut current_bytes = total_bytes;

    for entry in entries {
        if file_count <= IMAGE_CACHE_MAX_FILES && current_bytes <= IMAGE_CACHE_MAX_BYTES {
            break;
        }

        if fs::remove_file(&entry.path).is_ok() {
            file_count = file_count.saturating_sub(1);
            current_bytes = current_bytes.saturating_sub(entry.size);
        }
    }
}

/// Parse a `jfimage://poster/{item_id}?tag={tag}` URI.
/// Returns (image_type, item_id, tag) on success.
fn parse_jfimage_uri(uri: &str) -> Option<(String, String, String)> {
    // URI may look like:
    //   "jfimage://poster/abc123?tag=xyz"
    //   "jfimage:///poster/abc123?tag=xyz"            (triple-slash)
    //   "jfimage://localhost/poster/abc123?tag=xyz"    (some platforms)
    //   "http://jfimage.localhost/poster/abc123?tag=xyz" (Tauri rewrite)
    let path_and_query = if let Some(rest) = uri.strip_prefix("jfimage://") {
        rest.to_string()
    } else if let Some(pos) = uri.find("jfimage.localhost/") {
        uri[pos + "jfimage.localhost/".len()..].to_string()
    } else {
        return None;
    };

    let (path, query) = match path_and_query.split_once('?') {
        Some((p, q)) => (p, q),
        None => return None,
    };

    // Split into segments and filter out empty parts and "localhost"
    // to handle leading slashes or "localhost" prefix robustly.
    let segments: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty() && *s != "localhost")
        .collect();

    // We expect exactly two meaningful segments: [image_type, item_id]
    if segments.len() != 2 {
        return None;
    }
    let image_type = segments[0];
    let item_id = segments[1];

    if item_id.is_empty() {
        return None;
    }

    // query = "tag=xyz"
    let tag = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("tag="))?;
    if tag.is_empty() {
        return None;
    }

    Some((image_type.to_string(), item_id.to_string(), tag.to_string()))
}

/// Build the Jellyfin image API URL based on type.
fn jellyfin_image_url(server_url: &str, image_type: &str, item_id: &str) -> String {
    let (endpoint, max_width) = match image_type {
        "backdrop" => ("Backdrop", 1280),
        _ => ("Primary", 400), // poster and fallback
    };
    format!(
        "{}/Items/{}/Images/{}?maxWidth={}",
        server_url.trim_end_matches('/'), item_id, endpoint, max_width
    )
}

/// Fetch an image from Jellyfin and save to cache (async, for background use).
async fn fetch_and_cache_image_async(
    app_handle: tauri::AppHandle,
    http_client: reqwest::Client,
    server_url: String,
    token: String,
    image_type: String,
    item_id: String,
    tag: String,
    local_path: PathBuf,
) {
    let url = jellyfin_image_url(&server_url, &image_type, &item_id);

    let resp = match http_client
        .get(&url)
        .header("X-Emby-Token", &token)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            warn!(target: "jfimage", item_id = %item_id, error = %err, "Background image fetch failed");
            return;
        }
    };

    if !resp.status().is_success() {
        warn!(
            target: "jfimage",
            item_id = %item_id,
            status = %resp.status(),
            "Background image fetch returned non-success status"
        );
        return;
    }

    let bytes = match resp.bytes().await {
        Ok(b) => b.to_vec(),
        Err(err) => {
            warn!(target: "jfimage", item_id = %item_id, error = %err, "Failed to read image bytes");
            return;
        }
    };

    if let Err(err) = fs::write(&local_path, &bytes) {
        warn!(
            target: "jfimage",
            item_id = %item_id,
            path = %local_path.display(),
            error = %err,
            "Failed to write image cache file"
        );
        return;
    }

    if let Some(cache_dir) = local_path.parent() {
        cleanup_image_cache(cache_dir);
    }

    let _ = app_handle.emit(
        IMAGE_CACHED_EVENT,
        ImageCachedPayload {
            image_type,
            item_id,
            tag,
        },
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .register_uri_scheme_protocol("jfimage", |ctx, request| {
            let uri = request.uri().to_string();

            let (image_type, item_id, tag) = match parse_jfimage_uri(&uri) {
                Some(parsed) => parsed,
                None => {
                    return tauri::http::Response::builder()
                        .status(404)
                        .header("Content-Type", "image/webp")
                        .body(TRANSPARENT_PIXEL_WEBP.to_vec())
                        .unwrap();
                }
            };

            // Resolve cache directory
            let cache_dir = match ctx.app_handle().path().app_cache_dir() {
                Ok(dir) => dir.join("image_cache"),
                Err(_) => {
                    return tauri::http::Response::builder()
                        .status(500)
                        .header("Content-Type", "image/webp")
                        .body(TRANSPARENT_PIXEL_WEBP.to_vec())
                        .unwrap();
                }
            };
            let _ = fs::create_dir_all(&cache_dir);

            let local_path = cache_dir.join(format!("{}_{}.webp", item_id, tag));

            // Cache hit — serve from disk instantly (no network)
            if local_path.exists() {
                if let Ok(bytes) = fs::read(&local_path) {
                    return tauri::http::Response::builder()
                        .status(200)
                        .header("Content-Type", "image/webp")
                        .header("Cache-Control", "max-age=31536000, immutable")
                        .body(bytes)
                        .unwrap();
                }
            }

            // Cache miss — return placeholder immediately, fetch in background
            let state = match ctx.app_handle().try_state::<AppState>() {
                Some(s) => s,
                None => {
                    return tauri::http::Response::builder()
                        .status(200)
                        .header("Content-Type", "image/webp")
                        .body(TRANSPARENT_PIXEL_WEBP.to_vec())
                        .unwrap();
                }
            };

            let server_url = state.server_url.read().ok().and_then(|v| v.clone());
            let token = state.token.read().ok().and_then(|v| v.clone());

            if let (Some(server_url), Some(token)) = (server_url, token) {
                let http_client = state.http_client.clone();
                let app_handle = ctx.app_handle().clone();
                // Spawn non-blocking background fetch — image will be cached for next request
                tauri::async_runtime::spawn(fetch_and_cache_image_async(
                    app_handle,
                    http_client,
                    server_url,
                    token,
                    image_type,
                    item_id,
                    tag,
                    local_path,
                ));
            }

            // Return transparent placeholder while the image is being fetched
            tauri::http::Response::builder()
                .status(200)
                .header("Content-Type", "image/webp")
                .header("Cache-Control", "no-store")
                .body(TRANSPARENT_PIXEL_WEBP.to_vec())
                .unwrap()
        })
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;

            if let Ok(cache_root) = app.path().app_cache_dir() {
                let image_cache_dir = cache_root.join("image_cache");
                let _ = fs::create_dir_all(&image_cache_dir);
                cleanup_image_cache(&image_cache_dir);
            }

            let log_dir = app_data_dir.join("logs");
            let log_file_path = diagnostics::init_logging(&log_dir)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            info!(
                target: "bootstrap",
                app_data_dir = %app_data_dir.display(),
                log_file = %log_file_path.display(),
                "Application setup started"
            );

            // Initialize SQLite with WAL for concurrent read/write
            let db_path = app_data_dir.join("jfgoat.db");
            let conn = Connection::open(&db_path)?;
            // WAL: allows concurrent reads while the sync worker writes.
            let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
            // busy_timeout: wait up to 5s instead of instant SQLITE_BUSY errors.
            let _: i64 = conn.query_row("PRAGMA busy_timeout = 5000", [], |r| r.get(0))?;
            db::init_db(&conn)?;
            info!(target: "bootstrap", db_path = %db_path.display(), "Database initialized");

            // Create HTTP client with timeouts and pool tuning for large library sync
            let http_client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(std::time::Duration::from_secs(120))
                .connect_timeout(std::time::Duration::from_secs(15))
                .tcp_keepalive(std::time::Duration::from_secs(20))
                .pool_idle_timeout(std::time::Duration::from_secs(30))
                .pool_max_idle_per_host(1)
                .build()
                .expect("Failed to build HTTP client");

            // Create and manage AppState
            let app_state = AppState {
                db: Mutex::new(conn),
                http_client,
                server_url: RwLock::new(None),
                user_id: RwLock::new(None),
                token: RwLock::new(None),
                sync_status: RwLock::new(SyncStatus::Ready),
                user_data_refresh_running: AtomicBool::new(false),
            };
            app.manage(app_state);

            // ── MPV Player Setup (Windows) ─────────────────────────────────
            #[cfg(target_os = "windows")]
            {
                use raw_window_handle::HasWindowHandle;
                use raw_window_handle::RawWindowHandle;

                // Set DLL search directory so libmpv2 can find mpv-2.dll
                if let Ok(resource_dir) = app.path().resource_dir() {
                    mpv::set_mpv_dll_directory(&resource_dir);
                    info!(target: "mpv", dll_dir = %resource_dir.display(), "Configured MPV DLL search directory");
                }

                let window = app
                    .get_webview_window("main")
                    .expect("No 'main' window found");

                let parent_hwnd = match window
                    .window_handle()
                    .expect("Failed to get window handle")
                    .as_raw()
                {
                    RawWindowHandle::Win32(h) => h.hwnd.get() as isize,
                    _ => panic!("Expected Win32 window handle"),
                };

                let child_hwnd = mpv::create_mpv_child_window(parent_hwnd)
                    .expect("Failed to create mpv child window");

                let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
                let app_handle = app.handle().clone();

                mpv::spawn_mpv_thread(child_hwnd, cmd_rx, app_handle);

                app.manage(mpv::MpvState { cmd_tx, child_hwnd });

                // Resize mpv child window when the main window is resized
                let mpv_hwnd = child_hwnd;
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Resized(size) = event {
                        mpv::resize_mpv_window(mpv_hwnd, size.width, size.height);
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect_to_server,
            commands::login,
            commands::check_auth,
            commands::check_auth_offline,
            commands::logout,
            commands::search_items,
            commands::get_sync_status,
            commands::start_sync,
            commands::force_resync,
            commands::export_diagnostics,
            commands::get_recent_movies,
            commands::get_recent_series,
            commands::get_continue_watching,
            commands::get_latest_media,
            commands::get_item_by_id,
            commands::get_series_seasons,
            commands::get_season_episodes,
            commands::get_user_views,
            commands::get_resume_items,
            commands::get_next_up,
            commands::get_latest_items,
            commands::get_item_people,
            commands::get_similar_items,
            commands::save_homepage_cache,
            commands::load_homepage_cache,
            commands::get_user_preferences,
            commands::save_user_preferences,
            commands::mpv_play,
            commands::mpv_toggle_pause,
            commands::mpv_seek,
            commands::mpv_seek_absolute,
            commands::mpv_set_volume,
            commands::mpv_set_mute,
            commands::mpv_set_playback_rate,
            commands::mpv_set_subtitle_position,
            commands::mpv_set_video_scale,
            commands::mpv_set_audio_track,
            commands::mpv_set_subtitle_track,
            commands::mpv_stop,
            commands::get_media_streams,
            commands::get_item_chapters,
            commands::get_external_urls,
            commands::report_playback_lifecycle,
            commands::report_playback_stopped,
            commands::toggle_played,
            commands::toggle_favorite,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{jellyfin_image_url, parse_jfimage_uri};

    #[test]
    fn parse_jfimage_uri_supports_protocol_variants() {
        let direct = parse_jfimage_uri("jfimage://poster/item-1?tag=abc");
        assert_eq!(
            direct,
            Some((
                "poster".to_string(),
                "item-1".to_string(),
                "abc".to_string(),
            ))
        );

        let localhost = parse_jfimage_uri("jfimage://localhost/backdrop/item-2?tag=xyz");
        assert_eq!(
            localhost,
            Some((
                "backdrop".to_string(),
                "item-2".to_string(),
                "xyz".to_string(),
            ))
        );

        let rewritten = parse_jfimage_uri("http://jfimage.localhost/poster/item-3?tag=tag3");
        assert_eq!(
            rewritten,
            Some((
                "poster".to_string(),
                "item-3".to_string(),
                "tag3".to_string(),
            ))
        );
    }

    #[test]
    fn parse_jfimage_uri_rejects_invalid_shapes() {
        assert!(parse_jfimage_uri("jfimage://poster/item").is_none());
        assert!(parse_jfimage_uri("jfimage://poster/?tag=x").is_none());
        assert!(parse_jfimage_uri("https://example.com/cover?id=1").is_none());
    }

    #[test]
    fn jellyfin_image_url_uses_type_specific_sizes() {
        let poster = jellyfin_image_url("http://demo.local/", "poster", "item-1");
        assert_eq!(
            poster,
            "http://demo.local/Items/item-1/Images/Primary?maxWidth=400"
        );

        let backdrop = jellyfin_image_url("http://demo.local", "backdrop", "item-2");
        assert_eq!(
            backdrop,
            "http://demo.local/Items/item-2/Images/Backdrop?maxWidth=1280"
        );
    }
}
