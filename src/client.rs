//! HTTP client for ActivityPub protocol.
//!
//! This module provides functionality for interacting with ActivityPub servers
//! including fetching objects, collections, actors, and submitting activities to outboxes.
//! Implementation follows the W3C ActivityPub specification at https://www.w3.org/TR/activitypub/

use crate::httpsignature::{
    ComponentIdentifier, HttpSignature, SignatureAlgorithm, SignatureConfig, SignatureError,
};
use crate::{Activity, ActivityPubEntity, Collection, Object, ObjectOrLink};
use reqwest::{
    Client, Response,
    header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use url::Url;

/// Standard ActivityPub content type for requests
pub const ACTIVITYPUB_CONTENT_TYPE: &str = "application/activity+json";
/// JSON-LD ActivityStreams content type profile
pub const ACTIVITY_STREAMS_JSON_LD: &str =
    "application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\"";

/// Error type for ActivityPub client operations
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Failed to parse JSON: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Invalid URL: {0}")]
    UrlError(#[from] url::ParseError),

    #[error("Failed with status: {0}")]
    StatusError(reqwest::StatusCode),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid header value: {0}")]
    InvalidHeader(#[from] reqwest::header::InvalidHeaderValue),

    #[error("Signature error: {0}")]
    SignatureError(#[from] SignatureError),
}

/// Result type for ActivityPub client operations
pub type Result<T> = std::result::Result<T, ClientError>;

/// Configuration options for ActivityPub client
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// User agent to use for requests
    pub user_agent: String,
    /// Optional HTTP signature configuration for signed requests
    pub http_signature_config: Option<SignatureConfig>,
    /// Optional OAuth credentials
    pub oauth_token: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            user_agent: format!("ActivityPub-RS/0.1.0"),
            http_signature_config: None,
            oauth_token: None,
        }
    }
}

/// ActivityPub HTTP client to interact with ActivityPub servers
#[derive(Debug, Clone)]
pub struct ActivityPubClient {
    client: Client,
    config: ClientConfig,
}

impl ActivityPubClient {
    /// Create a new ActivityPub client with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(ClientConfig::default())
    }

    /// Create a new ActivityPub client with the specified configuration
    pub fn with_config(config: ClientConfig) -> Result<Self> {
        let client = Client::builder().user_agent(&config.user_agent).build()?;

        Ok(Self { client, config })
    }

    /// Get default headers for ActivityPub requests
    fn default_headers(&self) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static(ACTIVITYPUB_CONTENT_TYPE));

        // Add OAuth token if configured
        if let Some(token) = &self.config.oauth_token {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token))?,
            );
        }

        Ok(headers)
    }

    /// Sign a request using HTTP Signatures if configured
    fn sign_request(&self, request: &mut reqwest::Request) -> Result<()> {
        if let Some(config) = &self.config.http_signature_config {
            // Sign the request directly using the updated HttpSignature
            HttpSignature::sign_request(request, config)?;
        }

        Ok(())
    }

    /// Fetch an ActivityPub object from a URL
    pub async fn fetch_object(&self, url: &Url) -> Result<ActivityPubEntity> {
        let mut request = self
            .client
            .get(url.clone())
            .headers(self.default_headers()?)
            .build()?;

        // Sign the request if configured
        self.sign_request(&mut request)?;

        let response = self.client.execute(request).await?;
        self.handle_response(response).await
    }

    /// Fetch an actor profile
    pub async fn fetch_actor(&self, actor_id: &Url) -> Result<Object> {
        let entity = self.fetch_object(actor_id).await?;

        match entity {
            ActivityPubEntity::Object(object) => Ok(object),
            _ => Err(ClientError::MissingField(format!(
                "Expected actor object, but got a different entity type"
            ))),
        }
    }

    /// Fetch a collection of items
    pub async fn fetch_collection(&self, collection_url: &Url) -> Result<Collection> {
        let entity = self.fetch_object(collection_url).await?;

        match entity {
            ActivityPubEntity::Collection(collection) => Ok(collection),
            _ => Err(ClientError::MissingField(format!(
                "Expected collection, but got a different entity type"
            ))),
        }
    }

    /// Fetch actor's inbox
    pub async fn fetch_inbox(&self, actor: &Object) -> Result<Collection> {
        let inbox_url = actor
            .additional_properties
            .get("inbox")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClientError::MissingField("Actor missing inbox".into()))?;

        let inbox_url = Url::parse(inbox_url)?;
        self.fetch_collection(&inbox_url).await
    }

    /// Fetch actor's outbox
    pub async fn fetch_outbox(&self, actor: &Object) -> Result<Collection> {
        let outbox_url = actor
            .additional_properties
            .get("outbox")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClientError::MissingField("Actor missing outbox".into()))?;

        let outbox_url = Url::parse(outbox_url)?;
        self.fetch_collection(&outbox_url).await
    }

    /// Send an activity to an actor's inbox
    pub async fn send_to_inbox(&self, inbox_url: &Url, activity: &Activity) -> Result<()> {
        let mut request = self
            .client
            .post(inbox_url.clone())
            .headers(self.default_headers()?)
            .header(CONTENT_TYPE, ACTIVITYPUB_CONTENT_TYPE)
            .json(activity)
            .build()?;

        // Sign the request if configured
        self.sign_request(&mut request)?;

        let response = self.client.execute(request).await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(ClientError::StatusError(response.status()))
        }
    }

    /// Post an activity to the actor's outbox
    pub async fn post_to_outbox(&self, outbox_url: &Url, activity: &Activity) -> Result<Activity> {
        let mut request = self
            .client
            .post(outbox_url.clone())
            .headers(self.default_headers()?)
            .header(CONTENT_TYPE, ACTIVITYPUB_CONTENT_TYPE)
            .json(activity)
            .build()?;

        // Sign the request if configured
        self.sign_request(&mut request)?;

        let response = self.client.execute(request).await?;

        if !response.status().is_success() {
            return Err(ClientError::StatusError(response.status()));
        }

        let entity = self.handle_response(response).await?;

        match entity {
            ActivityPubEntity::Activity(activity) => Ok(activity),
            _ => Err(ClientError::MissingField(
                "Expected activity in response".into(),
            )),
        }
    }

    /// Follow another actor
    pub async fn follow(&self, actor: &Object, target: &Url) -> Result<Activity> {
        let outbox_url = actor
            .additional_properties
            .get("outbox")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ClientError::MissingField("Actor missing outbox".into()))?;

        let outbox_url = Url::parse(outbox_url)?;

        // Create a Follow activity
        let follow_activity = Activity {
            activity_type: crate::ActivityType::Follow,
            id: None, // Server will assign an ID
            name: None,
            summary: None,
            actor: Some(ObjectOrLink::Url(actor.id.clone().ok_or_else(|| {
                ClientError::MissingField("Actor missing id".into())
            })?)),
            object: Some(ObjectOrLink::Url(target.clone())),
            target: None,
            published: None,
            updated: None,
            additional_properties: std::collections::HashMap::new(),
        };

        self.post_to_outbox(&outbox_url, &follow_activity).await
    }

    /// Helper method to handle responses and parse them
    async fn handle_response(&self, response: Response) -> Result<ActivityPubEntity> {
        if !response.status().is_success() {
            return Err(ClientError::StatusError(response.status()));
        }

        let text = response.text().await?;
        let entity = crate::parse_activitypub_json(&text)?;

        Ok(entity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_actor() {
        // Request a new server from the pool
        let mut server = mockito::Server::new_async().await;

        // Use one of these addresses to configure your client
        let url = server.url();

        let mock_actor = r#"
        {
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Person",
            "id": "https://example.com/users/test",
            "name": "Test User",
            "preferredUsername": "test",
            "inbox": "https://example.com/users/test/inbox",
            "outbox": "https://example.com/users/test/outbox"
        }
        "#;

        let m = server
            .mock("GET", "/users/test")
            .with_status(200)
            .with_header("content-type", "application/activity+json")
            .with_body(mock_actor)
            .create_async()
            .await;

        let client = ActivityPubClient::new().unwrap();
        let url = Url::parse(&format!("{}/users/test", url)).unwrap();

        let actor = client.fetch_actor(&url).await.unwrap();

        assert_eq!(actor.object_type, crate::ObjectType::Person);
        assert_eq!(actor.name, Some("Test User".to_string()));
        assert!(
            actor
                .additional_properties
                .contains_key("preferredUsername")
        );
        assert!(actor.additional_properties.contains_key("inbox"));
        assert!(actor.additional_properties.contains_key("outbox"));
        m.assert_async().await;
    }

    #[tokio::test]
    async fn test_post_to_outbox() {
        // Request a new server from the pool
        let mut server = mockito::Server::new_async().await;

        // Use one of these addresses to configure your client
        let url = server.url();

        let request_activity = r#"
        {
            "type": "Create",
            "actor": "https://example.com/users/test",
            "object": {
                "type": "Note",
                "content": "Hello world"
            }
        }
        "#;

        let response_activity = r#"
        {
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Create",
            "id": "https://example.com/activities/123",
            "actor": "https://example.com/users/test",
            "object": {
                "type": "Note",
                "id": "https://example.com/notes/123",
                "content": "Hello world",
                "published": "2023-01-01T00:00:00Z"
            },
            "published": "2023-01-01T00:00:00Z"
        }
        "#;

        let m = server
            .mock("POST", "/users/test/outbox")
            .with_status(200)
            .with_header("content-type", "application/activity+json")
            .match_body(mockito::Matcher::Json(
                serde_json::from_str(request_activity).unwrap(),
            ))
            .with_body(response_activity)
            .create_async()
            .await;

        let client = ActivityPubClient::new().unwrap();
        let url = Url::parse(&format!("{}/users/test/outbox", url)).unwrap();

        let activity: Activity = serde_json::from_str(request_activity).unwrap();
        let result = client.post_to_outbox(&url, &activity).await.unwrap();

        assert_eq!(result.activity_type, crate::ActivityType::Create);
        assert_eq!(
            result.id,
            Some(Url::parse("https://example.com/activities/123").unwrap())
        );
        m.assert_async().await;
    }

    #[tokio::test]
    async fn test_with_http_signature() {
        // This test would require actual keys, so we'll just demonstrate the setup
        let ed25519_key = b"dummy_key_for_demonstration_only";

        let signature_config = SignatureConfig {
            algorithm: SignatureAlgorithm::Ed25519,
            parameters: crate::httpsignature::SignatureParameters::new(),
            key_id: "https://example.com/keys/1".to_string(),
            components: vec![
                ComponentIdentifier::Method,
                ComponentIdentifier::Path,
                ComponentIdentifier::Header("host".to_string()),
                ComponentIdentifier::Header("date".to_string()),
                ComponentIdentifier::Header("content-type".to_string()),
            ],
            private_key: ed25519_key.to_vec(),
        };

        let client_config = ClientConfig {
            user_agent: "ActivityPub-Client/1.0".to_string(),
            http_signature_config: Some(signature_config),
            oauth_token: None,
        };

        // In a real scenario, this client would sign requests with the configured key
        let _client = ActivityPubClient::with_config(client_config).unwrap();
    }
}
