use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};
use serde::Serialize;
use tracing::warn;

use crate::state::AppState;

/// 1x1 transparent WebP (lossless) — returned on errors so <img> fails gracefully.
pub const TRANSPARENT_PIXEL_WEBP: &[u8] = &[
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
pub fn cleanup_image_cache(cache_dir: &Path) {
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
pub fn parse_jfimage_uri(uri: &str) -> Option<(String, String, String)> {
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

    let segments: Vec<&str> = path
        .split('/')
        .filter(|s| !s.is_empty() && *s != "localhost")
        .collect();

    if segments.len() != 2 {
        return None;
    }
    let image_type = segments[0];
    let item_id = segments[1];

    if item_id.is_empty() {
        return None;
    }

    let tag = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("tag="))?;
    if tag.is_empty() {
        return None;
    }

    Some((image_type.to_string(), item_id.to_string(), tag.to_string()))
}

/// Build the Jellyfin image API URL based on type.
pub fn jellyfin_image_url(server_url: &str, image_type: &str, item_id: &str) -> String {
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
async fn fetch_and_cache_image_async<R: tauri::Runtime>(
    app_handle: tauri::AppHandle<R>,
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

pub fn register_jfimage_protocol<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_uri_scheme_protocol("jfimage", |ctx, request| {
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

        let server_url = state.server_url.read().clone();
        let token = state.token.read().clone();

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
