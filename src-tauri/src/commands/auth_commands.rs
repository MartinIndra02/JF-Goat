use tauri::State;

use crate::api::auth::{self, LoginResult, ServerPublicInfo, SessionInfo};
use crate::api::client::JellyfinClient;
use crate::db::servers;
use crate::error::JfgoatError;
use crate::state::AppState;

const KEYRING_SERVICE: &str = "com.jfgoat.client";
const KEYRING_USER: &str = "access_token";
const KEYRING_USER_PASSWORD: &str = "saved_password";

fn get_device_id(state: &AppState) -> Result<String, JfgoatError> {
    let db = state.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let device_id: String = db.query_row(
        "SELECT value FROM metadata WHERE key = 'device_id'",
        [],
        |row| row.get(0),
    )?;
    Ok(device_id)
}

fn keyring_store_token(token: &str) -> Result<(), JfgoatError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| JfgoatError::Internal(format!("Keyring error: {}", e)))?;
    entry
        .set_password(token)
        .map_err(|e| JfgoatError::Internal(format!("Keyring store error: {}", e)))?;
    Ok(())
}

fn keyring_load_token() -> Result<Option<String>, JfgoatError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| JfgoatError::Internal(format!("Keyring error: {}", e)))?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(JfgoatError::Internal(format!("Keyring load error: {}", e))),
    }
}

fn keyring_clear_token() -> Result<(), JfgoatError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| JfgoatError::Internal(format!("Keyring error: {}", e)))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(JfgoatError::Internal(format!("Keyring clear error: {}", e))),
    }
}



fn keyring_clear_password() -> Result<(), JfgoatError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER_PASSWORD)
        .map_err(|e| JfgoatError::Internal(format!("Keyring error: {}", e)))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(JfgoatError::Internal(format!("Keyring clear error: {}", e))),
    }
}

/// Cache token, server URL, and user ID in AppState after successful authentication.
fn update_app_state_after_auth(
    state: &AppState,
    token: String,
    server_url: &str,
    user_id: &str,
) -> Result<(), JfgoatError> {
    state.update_session(Some(token), Some(server_url.to_string()), Some(user_id.to_string()));
    Ok(())
}

#[tauri::command]
pub async fn connect_to_server(
    url: String,
    state: State<'_, AppState>,
    _app: tauri::AppHandle,
) -> Result<ServerPublicInfo, JfgoatError> {
    // Prevent credential bleed when switching servers/accounts.
    keyring_clear_token()?;
    keyring_clear_password()?;

    let device_id = get_device_id(&state)?;

    let parsed_url = reqwest::Url::parse(&url)
        .map_err(|e| JfgoatError::Auth(format!("Invalid URL format: {}", e)))?;
    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err(JfgoatError::Auth("URL scheme must be http or https".to_string()));
    }
    let clean_url = parsed_url.as_str().trim_end_matches('/').to_string();

    let jf_client = JellyfinClient::new(&state.http_client, &clean_url, &device_id);
    let info = auth::validate_server(&jf_client).await?;

    // Store server in DB
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::upsert_server(&db, &info.id, &info.name, &info.url)?;
    }

    // Cache server URL
    state.update_session(None, Some(info.url.clone()), None);

    Ok(info)
}

#[tauri::command]
pub async fn login(
    username: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<LoginResult, JfgoatError> {
    let device_id = get_device_id(&state)?;

    let server_url = state
        .server_url
        .read()
        .clone()
        .ok_or_else(|| JfgoatError::Auth("No server connected".to_string()))?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id);
    let (token, mut result) = auth::authenticate_by_name(&jf_client, &username, &password).await?;

    // Store token in OS credential store
    keyring_store_token(&token)?;

    // Clear legacy password if any
    let _ = keyring_clear_password();

    // Update DB with user info & get server name
    let server_name = {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::set_active_server(&db, &result.server_id)?;
        servers::update_server_user(&db, &result.server_id, &result.user_id, &result.username)?;
        let s = servers::get_active_server(&db)?
            .ok_or_else(|| JfgoatError::Internal("Active server not found after login".to_string()))?;
        s.name
    };

    // Cache in AppState
    update_app_state_after_auth(&state, token, &server_url, &result.user_id)?;

    result.server_name = server_name;
    result.server_url = server_url;

    Ok(result)
}

#[tauri::command]
pub async fn check_auth(
    state: State<'_, AppState>,
) -> Result<Option<SessionInfo>, JfgoatError> {
    // Check if we have an active server with user info
    let server = {
        let db = state.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::get_active_server(&db)?
    };

    let server = match server {
        Some(s) => s,
        None => return Ok(None),
    };

    // Need both user_id and server info
    let (user_id, username) = match (server.user_id, server.username) {
        (Some(uid), Some(uname)) => (uid, uname),
        _ => return Ok(None),
    };

    let device_id = get_device_id(&state)?;

    // Try to validate the stored token first
    if let Some(token) = keyring_load_token()? {
        let jf_client = JellyfinClient::new(&state.http_client, &server.url, &device_id)
            .with_token(&token);

        // Use /Users/Me instead of /System/Info — the latter requires admin
        // privileges and returns 403 for regular users, falsely invalidating
        // a perfectly good token.
        let resp = jf_client.get("/Users/Me").await;
        match resp {
            Ok(r) if r.status().is_success() => {
                // Token is valid — cache in AppState
                update_app_state_after_auth(&state, token, &server.url, &user_id)?;

                return Ok(Some(SessionInfo {
                    user_id,
                    username,
                    server_id: server.id,
                    server_name: server.name,
                    server_url: server.url,
                }));
            }
            Ok(r) if r.status() == reqwest::StatusCode::UNAUTHORIZED => {
                // Token is definitively invalid — clear it
                keyring_clear_token()?;
            }
            Ok(r) => {
                // Other unexpected HTTP status code (e.g. server error, gateway timeout)
                // Return an HTTP error to indicate verification failed, but don't log out.
                return Err(JfgoatError::Http(format!(
                    "Server returned status code {}",
                    r.status()
                )));
            }
            Err(e) => {
                // Network error — keep the token (it may still be valid).
                // Do not clear credentials, but return the error so the client knows
                // that verification failed instead of saying the session is invalid.
                return Err(e);
            }
        }
    }

    // Token missing or invalid — clear any legacy password and return None (auto-login disabled)
    let _ = keyring_clear_password();
    Ok(None)
}

/// Fast offline auth check — returns the stored session without network validation.
/// Used for instant startup; the frontend should call check_auth() in the background
/// to verify the token and redirect to /connect if it's invalid.
#[tauri::command]
pub async fn check_auth_offline(
    state: State<'_, AppState>,
) -> Result<Option<SessionInfo>, JfgoatError> {
    let server = {
        let db = state.db.read_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::get_active_server(&db)?
    };

    let server = match server {
        Some(s) => s,
        None => return Ok(None),
    };

    let (user_id, username) = match (server.user_id, server.username) {
        (Some(uid), Some(uname)) => (uid, uname),
        _ => return Ok(None),
    };

    // Check if a token exists but don't verify it over the network
    let token = match keyring_load_token()? {
        Some(t) => t,
        None => return Ok(None),
    };

    // Pre-populate AppState so homepage data commands work immediately
    state.update_session(Some(token), Some(server.url.clone()), Some(user_id.clone()));

    Ok(Some(SessionInfo {
        user_id,
        username,
        server_id: server.id,
        server_name: server.name,
        server_url: server.url,
    }))
}

#[tauri::command]
pub async fn logout(
    state: State<'_, AppState>,
) -> Result<(), JfgoatError> {
    // Clear OS credential store
    keyring_clear_token()?;
    keyring_clear_password()?;

    // Clear cached state
    state.update_session(None, None, None);

    // Clear active server in DB
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::clear_active_server(&db)?;
    }

    Ok(())
}
