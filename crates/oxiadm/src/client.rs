//! HTTP client for communicating with the adminservd API
//!
//! Replaces the direct AMQP messaging with authenticated HTTP calls.

use miette::{IntoDiagnostic, Result, miette};
use oxifed::messaging::{
    AnnounceActivityMessage, DomainCreateMessage, DomainInfo, DomainUpdateMessage,
    FollowActivityMessage, FollowInfo, KeyGenerateMessage, LikeActivityMessage, NoteCreateMessage,
    NoteUpdateMessage, ProfileCreateMessage, ProfileUpdateMessage, UserCreateMessage, UserInfo,
};
use reqwest::StatusCode;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// HTTP client for the admin API
pub struct AdminApiClient {
    client: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl AdminApiClient {
    /// Create a new admin API client. Refreshes the token if needed before creating.
    pub async fn new(base_url: &str, access_token: String) -> Result<Self> {
        let client = reqwest::Client::new();
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            access_token,
        })
    }

    /// Send an authenticated GET request and deserialize the JSON response
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .into_diagnostic()
            .map_err(|e| miette!("HTTP request failed: {}", e))?;

        Self::handle_response(response).await
    }

    /// Send an authenticated GET request with query parameters
    async fn get_with_query<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .query(query)
            .send()
            .await
            .into_diagnostic()
            .map_err(|e| miette!("HTTP request failed: {}", e))?;

        Self::handle_response(response).await
    }

    /// Send an authenticated POST request with a JSON body
    async fn post<B: Serialize>(&self, path: &str, body: &B) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .await
            .into_diagnostic()
            .map_err(|e| miette!("HTTP request failed: {}", e))?;

        Self::handle_status(response).await
    }

    /// Send an authenticated PUT request with a JSON body
    async fn put<B: Serialize>(&self, path: &str, body: &B) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .put(&url)
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .await
            .into_diagnostic()
            .map_err(|e| miette!("HTTP request failed: {}", e))?;

        Self::handle_status(response).await
    }

    /// Send an authenticated DELETE request
    async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .into_diagnostic()
            .map_err(|e| miette!("HTTP request failed: {}", e))?;

        Self::handle_status(response).await
    }

    /// Handle a response that should be deserialized as JSON
    async fn handle_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
        let status = response.status();

        if status == StatusCode::UNAUTHORIZED {
            return Err(miette!(
                help = "Your token may have expired. Try: oxiadm login --issuer-url <URL>",
                "Authentication failed (401 Unauthorized)"
            ));
        }

        if status == StatusCode::NOT_FOUND {
            let body: Value = response
                .json()
                .await
                .unwrap_or_else(|_| serde_json::json!({"error": "Not found"}));
            let msg = body["error"].as_str().unwrap_or("Not found");
            return Err(miette!("{}", msg));
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(miette!("API request failed ({}): {}", status, body));
        }

        response
            .json()
            .await
            .into_diagnostic()
            .map_err(|e| miette!("Failed to parse API response: {}", e))
    }

    /// Handle a response where we only care about the status
    async fn handle_status(response: reqwest::Response) -> Result<()> {
        let status = response.status();

        if status == StatusCode::UNAUTHORIZED {
            return Err(miette!(
                help = "Your token may have expired. Try: oxiadm login --issuer-url <URL>",
                "Authentication failed (401 Unauthorized)"
            ));
        }

        if !status.is_success() && status != StatusCode::ACCEPTED {
            let body = response.text().await.unwrap_or_default();
            return Err(miette!("API request failed ({}): {}", status, body));
        }

        Ok(())
    }

    // --- Domain operations ---

    pub async fn list_domains(&self) -> Result<Vec<DomainInfo>> {
        self.get("/api/v1/domains").await
    }

    pub async fn get_domain(&self, name: &str) -> Result<Option<DomainInfo>> {
        let path = format!("/api/v1/domains/{}", name);
        match self.get::<DomainInfo>(&path).await {
            Ok(d) => Ok(Some(d)),
            Err(e) if e.to_string().contains("Not found") => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn create_domain(&self, message: &DomainCreateMessage) -> Result<()> {
        self.post("/api/v1/domains", message).await
    }

    pub async fn update_domain(&self, message: &DomainUpdateMessage) -> Result<()> {
        let path = format!("/api/v1/domains/{}", message.domain);
        self.put(&path, message).await
    }

    pub async fn delete_domain(&self, name: &str, force: bool) -> Result<()> {
        let path = if force {
            format!("/api/v1/domains/{}?force=true", name)
        } else {
            format!("/api/v1/domains/{}", name)
        };
        self.delete(&path).await
    }

    // --- User operations ---

    pub async fn list_users(&self) -> Result<Vec<UserInfo>> {
        self.get("/api/v1/users").await
    }

    pub async fn get_user(&self, username: &str) -> Result<Option<UserInfo>> {
        let path = format!("/api/v1/users/{}", username);
        match self.get::<UserInfo>(&path).await {
            Ok(u) => Ok(Some(u)),
            Err(e) if e.to_string().contains("Not found") => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn create_user(&self, message: &UserCreateMessage) -> Result<()> {
        self.post("/api/v1/users", message).await
    }

    // --- Person operations ---

    pub async fn create_person(&self, message: &ProfileCreateMessage) -> Result<()> {
        self.post("/api/v1/persons", message).await
    }

    pub async fn update_person(&self, message: &ProfileUpdateMessage) -> Result<()> {
        let path = format!("/api/v1/persons/{}", message.subject);
        self.put(&path, message).await
    }

    pub async fn delete_person(&self, id: &str, force: bool) -> Result<()> {
        let path = if force {
            format!("/api/v1/persons/{}?force=true", id)
        } else {
            format!("/api/v1/persons/{}", id)
        };
        self.delete(&path).await
    }

    // --- Note operations ---

    pub async fn create_note(&self, message: &NoteCreateMessage) -> Result<()> {
        self.post("/api/v1/notes", message).await
    }

    pub async fn update_note(&self, message: &NoteUpdateMessage) -> Result<()> {
        let path = format!("/api/v1/notes/{}", message.id);
        self.put(&path, message).await
    }

    pub async fn delete_note(&self, id: &str, force: bool) -> Result<()> {
        let path = if force {
            format!("/api/v1/notes/{}?force=true", id)
        } else {
            format!("/api/v1/notes/{}", id)
        };
        self.delete(&path).await
    }

    // --- Activity operations ---

    pub async fn follow(&self, actor: &str, object: &str) -> Result<()> {
        let message = FollowActivityMessage::new(actor.to_string(), object.to_string());
        self.post("/api/v1/activities/follow", &message).await
    }

    pub async fn like(&self, actor: &str, object: &str) -> Result<()> {
        let message = LikeActivityMessage::new(actor.to_string(), object.to_string());
        self.post("/api/v1/activities/like", &message).await
    }

    pub async fn announce(
        &self,
        actor: &str,
        object: &str,
        to: Option<String>,
        cc: Option<String>,
    ) -> Result<()> {
        let message = AnnounceActivityMessage::new(actor.to_string(), object.to_string(), to, cc);
        self.post("/api/v1/activities/announce", &message).await
    }

    // --- Follow query operations ---

    pub async fn list_following(&self, actor: &str) -> Result<Vec<FollowInfo>> {
        self.get_with_query("/api/v1/following", &[("actor", actor)])
            .await
    }

    pub async fn list_followers(&self, actor: &str) -> Result<Vec<FollowInfo>> {
        self.get_with_query("/api/v1/followers", &[("actor", actor)])
            .await
    }

    // --- Key operations ---

    pub async fn generate_key(
        &self,
        actor: &str,
        algorithm: &str,
        key_size: Option<u32>,
    ) -> Result<()> {
        let message = KeyGenerateMessage::new(actor.to_string(), algorithm.to_string(), key_size);
        self.post("/api/v1/keys/generate", &message).await
    }
}
