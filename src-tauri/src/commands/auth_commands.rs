use tauri::State;

use crate::api::auth::{self, LoginResult, ServerPublicInfo, SessionInfo};
use crate::api::client::JellyfinClient;
use crate::db::servers;
use crate::error::JfgoatError;
use crate::state::AppState;

const KEYRING_SERVICE: &str = "com.jfgoat.client";
const KEYRING_USER: &str = "access_token";

fn get_device_id(state: &AppState) -> Result<String, JfgoatError> {
    let db = state.db.lock().map_err(|e| JfgoatError::Internal(e.to_string()))?;
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

#[tauri::command]
pub async fn connect_to_server(
    url: String,
    state: State<'_, AppState>,
    _app: tauri::AppHandle,
) -> Result<ServerPublicInfo, JfgoatError> {
    let device_id = get_device_id(&state)?;

    let jf_client = JellyfinClient::new(&state.http_client, &url, &device_id);
    let info = auth::validate_server(&jf_client).await?;

    // Store server in DB
    {
        let db = state.db.lock().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::upsert_server(&db, &info.id, &info.name, &info.url)?;
    }

    // Cache server URL
    {
        let mut server_url = state.server_url.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *server_url = Some(info.url.clone());
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

    // Update DB with user info
    {
        let db = state.db.lock().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::update_server_user(&db, &result.server_id, &result.user_id, &result.username)?;
    }

    // Cache in AppState
    {
        let mut cached_token = state.token.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *cached_token = Some(token);
    }
    {
        let mut user_id = state.user_id.write().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        *user_id = Some(result.user_id.clone());
    }

    Ok(result)
}

#[tauri::command]
pub async fn check_auth(
    state: State<'_, AppState>,
) -> Result<Option<SessionInfo>, JfgoatError> {
    // Check if we have an active server with user info
    let server = {
        let db = state.db.lock().map_err(|e| JfgoatError::Internal(e.to_string()))?;
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

    // Try to load token from OS credential store
    let token = match keyring_load_token()? {
        Some(t) => t,
        None => return Ok(None),
    };

    // Optionally verify token with a lightweight API call
    let device_id = get_device_id(&state)?;
    let jf_client = JellyfinClient::new(&state.http_client, &server.url, &device_id)
        .with_token(&token);

    let resp = jf_client.get("/System/Info").await;
    match resp {
        Ok(r) if r.status().is_success() => {
            // Token is valid — cache in AppState
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
        _ => {
            // Token invalid — clear it
            keyring_clear_token()?;
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
        let db = state.db.lock().map_err(|e| JfgoatError::Internal(e.to_string()))?;
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
        let db = state.db.lock().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::clear_active_server(&db)?;
    }

    Ok(())
}
