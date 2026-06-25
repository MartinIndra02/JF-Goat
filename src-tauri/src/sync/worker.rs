use std::time::Duration;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager};

use crate::state::{AppState, SyncStatus};
use crate::api::client::JellyfinClient;
use crate::api::media;
use crate::commands::media_commands::apply_user_data_refresh_batch;

const INCREMENTAL_REFRESH_INTERVAL_SECS: u64 = 240;
const INCREMENTAL_REFRESH_BATCH_SIZE: u32 = 1000;
const INCREMENTAL_REFRESH_MAX_PAGES: u32 = 200;

macro_rules! println {
    ($($arg:tt)*) => {
        tracing::info!(target: "sync", "{}", format_args!($($arg)*))
    };
}

macro_rules! eprintln {
    ($($arg:tt)*) => {
        tracing::error!(target: "sync", "{}", format_args!($($arg)*))
    };
}

pub(crate) fn ensure_incremental_refresh_worker(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };

    if state
        .user_data_refresh_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let app_handle = app.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(INCREMENTAL_REFRESH_INTERVAL_SECS)).await;

            let Some(state) = app_handle.try_state::<AppState>() else {
                break;
            };

            let is_ready = *state.sync_status.read() == SyncStatus::Ready;
            if !is_ready {
                continue;
            }

            match refresh_user_data_once(&state).await {
                Ok(updated) => {
                    if updated > 0 {
                        println!("Incremental user-data refresh updated {} rows", updated);
                    }
                }
                Err(err) => {
                    eprintln!("Incremental user-data refresh failed: {}", err);
                }
            }
        }

        if let Some(state) = app_handle.try_state::<AppState>() {
            state.user_data_refresh_running.store(false, Ordering::SeqCst);
        }
    });
}

async fn refresh_user_data_once(state: &AppState) -> Result<u32, String> {
    let (server_url, token, user_id, device_id) = state.get_connection_params().map_err(|e| e.to_string())?;
    let server_id = state.get_server_id().map_err(|e| e.to_string())?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id)
        .with_token(&token);

    let mut start_index = 0u32;
    let mut pages = 0u32;
    let mut total_updated = 0u32;
    let mut expected_total: Option<u32> = None;

    loop {
        let enable_total_count = start_index == 0;
        let response = media::fetch_user_items_userdata(
            &jf_client,
            &user_id,
            start_index,
            INCREMENTAL_REFRESH_BATCH_SIZE,
            enable_total_count,
        )
        .await
        .map_err(|e| e.to_string())?;

        if enable_total_count {
            expected_total = Some(response.total_record_count);
        }

        if response.items.is_empty() {
            break;
        }

        let updated = apply_user_data_refresh_batch(state, &server_id, &user_id, &response.items)
            .map_err(|e| e.to_string())?;
        total_updated += updated;

        pages += 1;
        if pages >= INCREMENTAL_REFRESH_MAX_PAGES {
            println!(
                "Incremental user-data refresh reached page cap ({})",
                INCREMENTAL_REFRESH_MAX_PAGES
            );
            break;
        }

        start_index = start_index.saturating_add(INCREMENTAL_REFRESH_BATCH_SIZE);
        if let Some(total) = expected_total {
            if start_index >= total {
                break;
            }
        }
    }

    Ok(total_updated)
}
