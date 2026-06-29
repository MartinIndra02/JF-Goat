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

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn from_hex(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::new();
    for i in (0..hex.len()).step_by(2) {
        let res = u8::from_str_radix(&hex[i..i + 2], 16).ok()?;
        bytes.push(res);
    }
    Some(bytes)
}

#[cfg(target_os = "windows")]
#[repr(C)]
#[derive(Copy, Clone)]
#[allow(non_snake_case)]
struct DATA_BLOB {
    cbData: u32,
    pbData: *mut u8,
}

#[cfg(target_os = "windows")]
#[link(name = "crypt32")]
#[link(name = "kernel32")]
extern "system" {
    fn CryptProtectData(
        pDataIn: *mut DATA_BLOB,
        pszDataDescr: *const u16,
        pOptionalEntropy: *mut DATA_BLOB,
        pvReserved: *mut std::ffi::c_void,
        pPromptStruct: *mut std::ffi::c_void,
        dwFlags: u32,
        pDataOut: *mut DATA_BLOB,
    ) -> i32;

    fn CryptUnprotectData(
        pDataIn: *mut DATA_BLOB,
        ppszDataDescr: *mut *mut u16,
        pOptionalEntropy: *mut DATA_BLOB,
        pvReserved: *mut std::ffi::c_void,
        pPromptStruct: *mut std::ffi::c_void,
        dwFlags: u32,
        pDataOut: *mut DATA_BLOB,
    ) -> i32;

    fn LocalFree(hmem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}

#[cfg(target_os = "windows")]
fn encrypt_token(token: &str) -> Result<String, JfgoatError> {
    use std::ptr;

    let input_bytes = token.as_bytes();
    let mut data_in = DATA_BLOB {
        cbData: input_bytes.len() as u32,
        pbData: input_bytes.as_ptr() as *mut u8,
    };
    let mut data_out = DATA_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    unsafe {
        let success = CryptProtectData(
            &mut data_in,
            ptr::null(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            &mut data_out,
        );

        if success == 0 {
            return Err(JfgoatError::Internal("DPAPI encryption failed".to_string()));
        }

        let bytes = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
        let hex_str = to_hex(bytes);
        LocalFree(data_out.pbData as _);
        Ok(hex_str)
    }
}

#[cfg(target_os = "windows")]
fn decrypt_token(hex_str: &str) -> Result<String, JfgoatError> {
    use std::ptr;

    let encrypted_bytes = from_hex(hex_str).ok_or_else(|| JfgoatError::Internal("Invalid hex in database token".to_string()))?;
    let mut data_in = DATA_BLOB {
        cbData: encrypted_bytes.len() as u32,
        pbData: encrypted_bytes.as_ptr() as *mut u8,
    };
    let mut data_out = DATA_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    unsafe {
        let success = CryptUnprotectData(
            &mut data_in,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            &mut data_out,
        );

        if success == 0 {
            return Err(JfgoatError::Internal("DPAPI decryption failed".to_string()));
        }

        let bytes = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
        let decrypted = String::from_utf8(bytes.to_vec())
            .map_err(|e| JfgoatError::Internal(format!("Invalid UTF-8 in decrypted token: {}", e)))?;
        LocalFree(data_out.pbData as _);
        Ok(decrypted)
    }
}

#[cfg(not(target_os = "windows"))]
fn encrypt_token(token: &str) -> Result<String, JfgoatError> {
    let user = std::env::var("USER").unwrap_or_else(|_| "default_user".to_string());
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "default_host".to_string());
    let key = format!("jfgoat-{}-{}", user, host);
    let key_bytes = key.as_bytes();

    let encrypted: Vec<u8> = token
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ key_bytes[i % key_bytes.len()])
        .collect();
    Ok(to_hex(&encrypted))
}

#[cfg(not(target_os = "windows"))]
fn decrypt_token(hex_str: &str) -> Result<String, JfgoatError> {
    let user = std::env::var("USER").unwrap_or_else(|_| "default_user".to_string());
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "default_host".to_string());
    let key = format!("jfgoat-{}-{}", user, host);
    let key_bytes = key.as_bytes();

    let encrypted_bytes = from_hex(hex_str).ok_or_else(|| JfgoatError::Internal("Invalid hex in database token".to_string()))?;
    let decrypted: Vec<u8> = encrypted_bytes
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ key_bytes[i % key_bytes.len()])
        .collect();
    String::from_utf8(decrypted)
        .map_err(|e| JfgoatError::Internal(format!("Invalid UTF-8 in decrypted token: {}", e)))
}

fn db_store_token(state: &AppState, token: &str) -> Result<(), JfgoatError> {
    let encrypted = encrypt_token(token)?;
    let db = state
        .db
        .write_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    db.execute(
        "INSERT INTO metadata (key, value)
         VALUES ('session_token', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![encrypted],
    )?;
    Ok(())
}

fn db_load_token(state: &AppState) -> Result<Option<String>, JfgoatError> {
    let db = state
        .db
        .read_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    let maybe_token: Result<String, rusqlite::Error> = db.query_row(
        "SELECT value FROM metadata WHERE key = 'session_token'",
        [],
        |row| row.get(0),
    );
    match maybe_token {
        Ok(token_hex) => {
            let decrypted = decrypt_token(&token_hex)?;
            Ok(Some(decrypted))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(JfgoatError::Database(e.to_string())),
    }
}

fn db_clear_token(state: &AppState) -> Result<(), JfgoatError> {
    let db = state
        .db
        .write_conn()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;
    db.execute("DELETE FROM metadata WHERE key = 'session_token'", [])?;
    Ok(())
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
    db_clear_token(&state)?;

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
    let check_result = {
        let attempts = state.login_attempts.lock();
        if let Some(&(count, last_failure_time)) = attempts.get(&username) {
            if count > 0 {
                let delay_secs = 2u64.pow(count - 1).min(60);
                let elapsed = std::time::Instant::now().duration_since(last_failure_time).as_secs();
                if elapsed < delay_secs {
                    Some(delay_secs - elapsed)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    };

    if let Some(wait_remaining) = check_result {
        return Err(JfgoatError::Auth(format!(
            "Too many login attempts. Please wait {} seconds.",
            wait_remaining
        )));
    }

    let device_id = get_device_id(&state)?;

    let server_url = state
        .server_url
        .read()
        .clone()
        .ok_or_else(|| JfgoatError::Auth("No server connected".to_string()))?;

    let jf_client = JellyfinClient::new(&state.http_client, &server_url, &device_id);
    let (token, mut result) = match auth::authenticate_by_name(&jf_client, &username, &password).await {
        Ok(res) => {
            state.login_attempts.lock().remove(&username);
            res
        }
        Err(err) => {
            let mut attempts = state.login_attempts.lock();
            let entry = attempts.entry(username.clone()).or_insert((0, std::time::Instant::now()));
            entry.0 += 1;
            entry.1 = std::time::Instant::now();
            return Err(err);
        }
    };

    // Store token in OS credential store & DB fallback
    keyring_store_token(&token)?;
    db_store_token(&state, &token)?;

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

    // Try to load token from keyring first, fallback to DB if missing or error
    let token_opt = match keyring_load_token() {
        Ok(Some(token)) => Some(token),
        _ => db_load_token(&state)?,
    };

    if let Some(token) = token_opt {
        let jf_client = JellyfinClient::new(&state.http_client, &server.url, &device_id)
            .with_token(&token);

        // Use /Users/Me instead of /System/Info — the latter requires admin
        // privileges and returns 403 for regular users, falsely invalidating
        // a perfectly good token.
        let resp = jf_client.get("/Users/Me").await;
        match resp {
            Ok(r) if r.status().is_success() => {
                // Token is valid — cache in AppState
                update_app_state_after_auth(&state, token.clone(), &server.url, &user_id)?;

                // Ensure token is synced to both stores
                let _ = keyring_store_token(&token);
                let _ = db_store_token(&state, &token);

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
                db_clear_token(&state)?;
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
    let token = match keyring_load_token() {
        Ok(Some(t)) => Some(t),
        _ => db_load_token(&state)?,
    };

    let token = match token {
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
    // Clear OS credential store & DB fallback
    keyring_clear_token()?;
    keyring_clear_password()?;
    db_clear_token(&state)?;

    // Clear cached state
    state.update_session(None, None, None);

    // Clear active server in DB
    {
        let db = state.db.write_conn().map_err(|e| JfgoatError::Internal(e.to_string()))?;
        servers::clear_active_server(&db)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::collections::HashMap;
    use std::time::Instant;

    #[test]
    fn test_login_rate_limiting_delay_calculation() {
        let attempts_map = Mutex::new(HashMap::new());
        let username = "testuser".to_string();

        let check_initial = {
            let attempts = attempts_map.lock();
            if let Some(&(count, last_failure_time)) = attempts.get(&username) {
                if count > 0 {
                    let delay_secs = 2u64.pow(count - 1).min(60);
                    let elapsed = Instant::now().duration_since(last_failure_time).as_secs();
                    if elapsed < delay_secs {
                        Some(delay_secs - elapsed)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };
        assert_eq!(check_initial, None);

        {
            let mut attempts = attempts_map.lock();
            attempts.insert(username.clone(), (1, Instant::now()));
        }

        let check_after_1_failure = {
            let attempts = attempts_map.lock();
            if let Some(&(count, last_failure_time)) = attempts.get(&username) {
                if count > 0 {
                    let delay_secs = 2u64.pow(count - 1).min(60);
                    let elapsed = Instant::now().duration_since(last_failure_time).as_secs();
                    if elapsed < delay_secs {
                        Some(delay_secs - elapsed)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };
        assert!(check_after_1_failure.is_some());
        assert!(check_after_1_failure.unwrap() <= 1);

        {
            let mut attempts = attempts_map.lock();
            attempts.insert(username.clone(), (5, Instant::now()));
        }

        let check_after_5_failures = {
            let attempts = attempts_map.lock();
            if let Some(&(count, last_failure_time)) = attempts.get(&username) {
                if count > 0 {
                    let delay_secs = 2u64.pow(count - 1).min(60);
                    let elapsed = Instant::now().duration_since(last_failure_time).as_secs();
                    if elapsed < delay_secs {
                        Some(delay_secs - elapsed)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };
        assert!(check_after_5_failures.is_some());
        assert!(check_after_5_failures.unwrap() <= 16);
    }
}
