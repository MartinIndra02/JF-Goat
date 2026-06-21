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
    let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
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

fn keyring_store_password(password: &str) -> Result<(), JfgoatError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER_PASSWORD)
        .map_err(|e| JfgoatError::Internal(format!("Keyring error: {}", e)))?;
    entry
        .set_password(password)
        .map_err(|e| JfgoatError::Internal(format!("Keyring store error: {}", e)))?;
    Ok(())
}

fn keyring_load_password() -> Result<Option<String>, JfgoatError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER_PASSWORD)
        .map_err(|e| JfgoatError::Internal(format!("Keyring error: {}", e)))?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(JfgoatError::Internal(format!("Keyring load error: {}", e))),
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
    {
        let mut cached_token = state.token.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *cached_token = Some(token);
    }
    {
        let mut cached_url = state.server_url.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *cached_url = Some(server_url.to_string());
    }
    {
        let mut cached_uid = state.user_id.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *cached_uid = Some(user_id.to_string());
    }
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

    let jf_client = JellyfinClient::new(&state.http_client, &url, &device_id);
    let info = auth::validate_server(&jf_client).await?;

    // Store server in DB
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::upsert_server(&db, &info.id, &info.name, &info.url)?;
    }

    // Cache server URL
    {
        let mut server_url = state.server_url.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *server_url = Some(info.url.clone());
    }
    {
        let mut cached_token = state.token.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *cached_token = None;
    }
    {
        let mut cached_uid = state.user_id.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *cached_uid = None;
    }

    Ok(info)
}

#[tauri::command]
pub async fn login(
    username: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<LoginResult, JfgoatError> {
    let device_id = get_device_id(&state)?;

    let server_url = {
        let url = state.server_url.read().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        url.clone().ok_or_else(|| JfgoatError::Auth("No server connected".to_string()))?
    };

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id);
    let (token, result) = auth::authenticate_by_name(&jf_client, &username, &password).await?;

    // Store token in OS credential store
    keyring_store_token(&token)?;

    // Store password in OS credential store for auto-login when token expires.
    // The OS keyring (GNOME Keyring / macOS Keychain / Windows Credential Manager)
    // encrypts stored credentials; they are never persisted in plaintext.
    keyring_store_password(&password)?;

    // Update DB with user info
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::set_active_server(&db, &result.server_id)?;
        servers::update_server_user(&db, &result.server_id, &result.user_id, &result.username)?;
    }

    // Cache in AppState
    update_app_state_after_auth(&state, token, &server_url, &result.user_id)?;

    Ok(result)
}

#[tauri::command]
pub async fn check_auth(
    state: State<'_, AppState>,
) -> Result<Option<SessionInfo>, JfgoatError> {
    // Check if we have an active server with user info
    let server = {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
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
                // Token is definitively invalid — clear it and fall through to auto-login
                keyring_clear_token()?;
            }
            _ => {
                // Network error or unexpected status — keep the token (it may still
                // be valid) and fall through to auto-login attempt.
                // Don't clear credentials on transient failures.
            }
        }
    }

    // Token missing or invalid — attempt auto-login with stored credentials
    let password = match keyring_load_password()? {
        Some(p) => p,
        None => return Ok(None),
    };

    let jf_client = JellyfinClient::new(&state.http_client, &server.url, &device_id);
    match auth::authenticate_by_name(&jf_client, &username, &password).await {
        Ok((new_token, result)) => {
            // Auto-login succeeded — store new token and update AppState
            keyring_store_token(&new_token)?;
            {
                let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
                servers::set_active_server(&db, &result.server_id)?;
                servers::update_server_user(&db, &result.server_id, &result.user_id, &result.username)?;
            }
            update_app_state_after_auth(&state, new_token, &server.url, &result.user_id)?;
            Ok(Some(SessionInfo {
                user_id: result.user_id,
                username: result.username,
                server_id: result.server_id,
                server_name: server.name,
                server_url: server.url,
            }))
        }
        Err(JfgoatError::Auth(_)) => {
            // Invalid credentials — clear stored password to avoid retry loops
            keyring_clear_password()?;
            Ok(None)
        }
        Err(_) => {
            // Network or server error — keep credentials for next attempt
            Ok(None)
        }
    }
}

/// Fast offline auth check — returns the stored session without network validation.
/// Used for instant startup; the frontend should call check_auth() in the background
/// to verify the token and redirect to /connect if it's invalid.
#[tauri::command]
pub async fn check_auth_offline(
    state: State<'_, AppState>,
) -> Result<Option<SessionInfo>, JfgoatError> {
    let server = {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
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
    {
        let mut cached_token = state.token.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *cached_token = Some(token);
    }
    {
        let mut cached_url = state.server_url.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *cached_url = Some(server.url.clone());
    }
    {
        let mut cached_uid = state.user_id.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *cached_uid = Some(user_id.clone());
    }

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
    {
        let mut token = state.token.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *token = None;
    }
    {
        let mut user_id = state.user_id.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *user_id = None;
    }
    {
        let mut server_url = state.server_url.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *server_url = None;
    }

    // Clear active server in DB
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::clear_active_server(&db)?;
    }

    Ok(())
}
