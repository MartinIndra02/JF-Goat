use reqwest::Client;

use crate::error::JfgoatError;

pub struct JellyfinClient {
    pub client: Client,
    pub base_url: String,
    pub device_id: String,
    pub token: Option<String>,
}

impl JellyfinClient {
    pub fn new(client: &Client, base_url: &str, device_id: &str) -> Self {
        Self {
            client: client.clone(),
            base_url: base_url.trim_end_matches('/').to_string(),
            device_id: device_id.to_string(),
            token: None,
        }
    }

    pub fn with_token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    pub fn auth_header(&self) -> String {
        let device_name = match std::env::consts::OS {
            "windows" => "Windows PC",
            "macos" => "macOS PC",
            "linux" => "Linux PC",
            os => os,
        };
        let mut header = format!(
            "MediaBrowser Client=\"jfgoat\", Device=\"{}\", DeviceId=\"{}\", Version=\"0.1.0\"",
            device_name, self.device_id
        );
        if let Some(ref token) = self.token {
            header.push_str(&format!(", Token=\"{}\"", token));
        }
        header
    }

    pub async fn get(&self, path: &str) -> Result<reqwest::Response, JfgoatError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;
        Ok(resp)
    }

    pub async fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<reqwest::Response, JfgoatError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(body)
            .send()
            .await?;
        Ok(resp)
    }

    pub async fn post_empty(&self, path: &str) -> Result<reqwest::Response, JfgoatError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;
        Ok(resp)
    }

    pub async fn delete(&self, path: &str) -> Result<reqwest::Response, JfgoatError> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .delete(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;
        Ok(resp)
    }
}
