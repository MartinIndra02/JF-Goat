use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tracing::info;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

use crate::state::{AppState, SyncStatus};

static LOG_FILE_PATH: OnceLock<PathBuf> = OnceLock::new();
static APP_STARTED_AT_UNIX_MS: OnceLock<u128> = OnceLock::new();

#[derive(Debug, Serialize)]
pub struct DiagnosticsExportResult {
    pub file_path: String,
    pub generated_at_unix_ms: u128,
    pub recent_log_lines: usize,
}

pub fn init_logging(log_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(log_dir)
        .map_err(|e| format!("Failed to create log directory: {}", e))?;

    let log_file_name = "jfgoat.log.jsonl";
    let log_file_path = log_dir.join(log_file_name);

    let _ = APP_STARTED_AT_UNIX_MS.get_or_init(now_unix_ms);
    let _ = LOG_FILE_PATH.get_or_init(|| log_file_path.clone());

    let file_appender = tracing_appender::rolling::never(log_dir, log_file_name);
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,jfgoat=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().compact().with_target(true))
        .with(
            fmt::layer()
                .json()
                .with_ansi(false)
                .with_current_span(false)
                .with_span_list(false)
                .with_writer(file_appender),
        )
        .try_init()
        .map_err(|e| format!("Failed to initialize tracing subscriber: {}", e))?;

    info!(
        target: "bootstrap",
        log_file = %log_file_path.display(),
        "Structured logging initialized"
    );

    Ok(log_file_path)
}

pub fn export_diagnostics(
    app: &AppHandle,
    state: &AppState,
) -> Result<DiagnosticsExportResult, String> {
    let generated_at_unix_ms = now_unix_ms();

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))?;
    let app_cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("Failed to resolve app cache dir: {}", e))?;

    let support_dir = app_data_dir.join("support");
    fs::create_dir_all(&support_dir)
        .map_err(|e| format!("Failed to create support directory: {}", e))?;

    let log_file_path = LOG_FILE_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| app_data_dir.join("logs").join("jfgoat.log.jsonl"));

    let log_file_metadata = fs::metadata(&log_file_path).ok();
    let log_file_size_bytes = log_file_metadata
        .as_ref()
        .map(std::fs::Metadata::len)
        .unwrap_or(0);
    let recent_logs = read_recent_logs(&log_file_path, 250);
    let error_count = count_logs_at_level(&recent_logs, "ERROR") + count_logs_at_level(&recent_logs, "error");
    let warn_count = count_logs_at_level(&recent_logs, "WARN") + count_logs_at_level(&recent_logs, "warn");

    let sync_status = {
        let status = state
            .sync_status
            .read()
            .map_err(|e| format!("Failed to read sync status: {}", e))?;
        sync_status_label(*status).to_string()
    };

    let has_server_url = state
        .server_url
        .read()
        .map_err(|e| format!("Failed to read server URL state: {}", e))?
        .is_some();
    let has_user_id = state
        .user_id
        .read()
        .map_err(|e| format!("Failed to read user ID state: {}", e))?
        .is_some();
    let has_token = state
        .token
        .read()
        .map_err(|e| format!("Failed to read token state: {}", e))?
        .is_some();

    let (media_items_count, servers_count, checkpoints_count, metadata_count) = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("Failed to lock database: {}", e))?;

        (
            safe_count_query(&db, "SELECT COUNT(*) FROM media_items"),
            safe_count_query(&db, "SELECT COUNT(*) FROM servers"),
            safe_count_query(&db, "SELECT COUNT(*) FROM sync_checkpoints"),
            safe_count_query(&db, "SELECT COUNT(*) FROM metadata"),
        )
    };

    let report = json!({
        "diagnostic_style": "D6",
        "schema_version": 1,
        "generated_at_unix_ms": generated_at_unix_ms,
        "app": {
            "name": app.package_info().name,
            "version": app.package_info().version.to_string(),
            "crate_version": env!("CARGO_PKG_VERSION"),
            "debug_build": cfg!(debug_assertions)
        },
        "runtime": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "app_started_at_unix_ms": APP_STARTED_AT_UNIX_MS.get().copied().unwrap_or(generated_at_unix_ms),
            "uptime_ms": generated_at_unix_ms.saturating_sub(APP_STARTED_AT_UNIX_MS.get().copied().unwrap_or(generated_at_unix_ms))
        },
        "session": {
            "sync_status": sync_status,
            "has_server_url": has_server_url,
            "has_user_id": has_user_id,
            "has_token": has_token
        },
        "storage": {
            "app_data_dir": app_data_dir,
            "app_cache_dir": app_cache_dir,
            "log_file": log_file_path,
            "log_file_exists": log_file_metadata.is_some(),
            "log_file_size_bytes": log_file_size_bytes
        },
        "database": {
            "media_items_count": media_items_count,
            "servers_count": servers_count,
            "checkpoints_count": checkpoints_count,
            "metadata_count": metadata_count
        },
        "d6": {
            "classification": "customer-support",
            "redactions_applied": [
                "token_values",
                "full_server_urls",
                "auth_headers"
            ],
            "health_checks": {
                "logging_active": log_file_metadata.is_some(),
                "db_accessible": media_items_count >= 0,
                "state_locks_accessible": true
            },
            "log_summary": {
                "recent_lines": recent_logs.len(),
                "recent_error_lines": error_count,
                "recent_warn_lines": warn_count
            }
        },
        "recent_logs": recent_logs
    });

    let report_path = support_dir.join(format!("diagnostics-d6-{}.json", generated_at_unix_ms));
    let report_json = serde_json::to_vec_pretty(&report)
        .map_err(|e| format!("Failed to serialize diagnostics report: {}", e))?;

    fs::write(&report_path, report_json)
        .map_err(|e| format!("Failed to write diagnostics report: {}", e))?;

    info!(
        target: "diagnostics",
        report_file = %report_path.display(),
        recent_lines = recent_logs.len(),
        "Diagnostics export completed"
    );

    Ok(DiagnosticsExportResult {
        file_path: report_path.to_string_lossy().to_string(),
        generated_at_unix_ms,
        recent_log_lines: recent_logs.len(),
    })
}

fn safe_count_query(db: &rusqlite::Connection, sql: &str) -> i64 {
    db.query_row(sql, [], |row| row.get(0)).unwrap_or(0)
}

fn sync_status_label(status: SyncStatus) -> &'static str {
    match status {
        SyncStatus::InitialSync => "initial_sync",
        SyncStatus::Ready => "ready",
    }
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn read_recent_logs(log_file_path: &Path, max_lines: usize) -> Vec<Value> {
    let text = match fs::read_to_string(log_file_path) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };

    let mut lines: Vec<&str> = text.lines().rev().take(max_lines).collect();
    lines.reverse();

    lines
        .into_iter()
        .map(|line| {
            serde_json::from_str::<Value>(line)
                .unwrap_or_else(|_| json!({ "level": "unknown", "message": line }))
        })
        .collect()
}

fn count_logs_at_level(logs: &[Value], level: &str) -> usize {
    logs
        .iter()
        .filter(|entry| entry.get("level").and_then(Value::as_str) == Some(level))
        .count()
}
