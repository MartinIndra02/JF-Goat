use serde::{Deserialize, Serialize};

use crate::api::client::JellyfinClient;
use crate::error::JfgoatError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerPublicInfo {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "ServerName")]
    pub name: String,
    #[serde(alias = "Version")]
    pub version: String,
    #[serde(skip_deserializing)]
    pub url: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct LoginResult {
    pub user_id: String,
    pub username: String,
    pub server_id: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SessionInfo {
    pub user_id: String,
    pub username: String,
    pub server_id: String,
    pub server_name: String,
    pub server_url: String,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    #[serde(alias = "AccessToken")]
    access_token: String,
    #[serde(alias = "ServerId")]
    server_id: String,
    #[serde(alias = "User")]
    user: AuthUser,
}

#[derive(Debug, Deserialize)]
struct AuthUser {
    #[serde(alias = "Id")]
    id: String,
    #[serde(alias = "Name")]
    name: String,
}

pub async fn validate_server(client: &JellyfinClient) -> Result<ServerPublicInfo, JfgoatError> {
    let resp = client.get("/System/Info/Public").await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Server returned status {}",
            resp.status()
        )));
    }

    let mut info: ServerPublicInfo = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse server response: {}", e))
    })?;
    info.url = client.base_url.clone();

    Ok(info)
}

pub async fn authenticate_by_name(
    client: &JellyfinClient,
    username: &str,
    password: &str,
) -> Result<(String, LoginResult), JfgoatError> {
    let body = serde_json::json!({
        "Username": username,
        "Pw": password
    });

    let resp = client.post_json("/Users/AuthenticateByName", &body).await?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(JfgoatError::Auth("Invalid username or password".to_string()));
    }

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Authentication failed with status {}",
            resp.status()
        )));
    }

    let auth: AuthResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse auth response: {}", e))
    })?;

    let token = auth.access_token;
    let result = LoginResult {
        user_id: auth.user.id,
        username: auth.user.name,
        server_id: auth.server_id,
    };

    Ok((token, result))
}
