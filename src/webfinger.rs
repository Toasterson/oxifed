//! WebFinger client implementation based on RFC 7565
//!
//! This module provides functionality for discovering information about entities
//! identified by URIs using the WebFinger protocol as specified in RFC 7565:
//! https://datatracker.ietf.org/doc/html/rfc7565

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use url::Url;

/// Error types specific to WebFinger operations
#[derive(Debug, Error)]
pub enum WebFingerError {
    #[error("Invalid resource URI: {0}")]
    InvalidResource(String),

    #[error("Failed to construct WebFinger URL: {0}")]
    UrlConstructionError(#[from] url::ParseError),

    #[error("HTTP request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("HTTP error response: {0}")]
    HttpError(reqwest::StatusCode),

    #[error("Host extraction failed for resource: {0}")]
    HostExtractionFailed(String),
}

/// Result type for WebFinger operations
pub type Result<T> = std::result::Result<T, WebFingerError>;

/// Link object as defined in RFC 7565
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    /// The "rel" parameter of a link contains a URI that
    /// describes the type of resource being linked
    pub rel: String,

    /// The "href" parameter of a link contains the target URI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,

    /// The "type" parameter of a link contains the media type of the target resource
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,

    /// The "titles" parameter of a link contains human-readable labels for the link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titles: Option<HashMap<String, String>>,

    /// The "properties" parameter of a link contains additional information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>,
}

/// JSON Resource Descriptor (JRD) as defined in RFC 7565
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JrdResource {
    /// The "subject" parameter identifies the entity that the JRD describes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    /// The "aliases" parameter is an array of zero or more URI strings
    /// that identify the same entity as the "subject" URI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,

    /// The "properties" parameter contains additional information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, serde_json::Value>>,

    /// The "links" parameter contains links related to the entity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<Link>>,
}

impl JrdResource {
    /// Find a link with the specified relation type
    pub fn find_link(&self, rel: &str) -> Option<&Link> {
        self.links
            .as_ref()
            .and_then(|links| links.iter().find(|link| link.rel == rel))
    }

    /// Find all links with the specified relation type
    pub fn find_links(&self, rel: &str) -> Vec<&Link> {
        self.links.as_ref().map_or(Vec::new(), |links| {
            links.iter().filter(|link| link.rel == rel).collect()
        })
    }
}

/// WebFinger client implementation
#[derive(Debug, Clone)]
pub struct WebFingerClient {
    client: Client,
}

impl WebFingerClient {
    /// Create a new WebFinger client
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Create a new WebFinger client with a custom HTTP client
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }

    /// Perform a WebFinger lookup for a resource
    ///
    /// # Arguments
    /// * `resource` - The URI identifying the entity to look up. Must begin with
    ///   "acct:", "http:", or "https:" as per RFC 7565
    /// * `rel` - Optional relation types to filter the result links
    ///
    /// # Returns
    /// A JRD Resource containing information about the requested resource
    pub async fn finger(&self, resource: &str, rel: Option<&[&str]>) -> Result<JrdResource> {
        // Validate the resource URI according to RFC 7565
        if !resource.starts_with("acct:")
            && !resource.starts_with("http:")
            && !resource.starts_with("https:")
        {
            return Err(WebFingerError::InvalidResource(format!(
                "Resource '{}' must begin with 'acct:', 'http:', or 'https:'",
                resource
            )));
        }

        // Extract host from the resource URI
        let host = self.extract_host(resource)?;

        // Construct the WebFinger URL
        let mut webfinger_url = Url::parse(&format!("https://{}/.well-known/webfinger", host))?;

        // Add query parameters
        let mut query_pairs = webfinger_url.query_pairs_mut();
        query_pairs.append_pair("resource", resource);

        // Add optional rel parameter(s)
        if let Some(rel_values) = rel {
            for r in rel_values {
                query_pairs.append_pair("rel", r);
            }
        }
        drop(query_pairs);

        // Make the request
        let response = self
            .client
            .get(webfinger_url)
            .header("Accept", "application/jrd+json, application/json")
            .send()
            .await?;

        // Check for success and parse response
        if !response.status().is_success() {
            return Err(WebFingerError::HttpError(response.status()));
        }

        let jrd = response.json::<JrdResource>().await?;

        Ok(jrd)
    }

    /// Extract the host from a resource URI
    fn extract_host(&self, resource: &str) -> Result<String> {
        if resource.starts_with("acct:") {
            // For acct: URIs, the host is the domain part after the @ symbol
            let parts: Vec<&str> = resource.splitn(2, "@").collect();
            if parts.len() != 2 {
                return Err(WebFingerError::HostExtractionFailed(format!(
                    "Invalid acct URI format: {}",
                    resource
                )));
            }

            // For acct:user@example.com, return example.com
            // Handle potential additional parts like acct:user@example.com:port/path
            let domain = parts[1].split([':', '/']).next().ok_or_else(|| {
                WebFingerError::HostExtractionFailed(format!(
                    "Could not extract domain from acct URI: {}",
                    resource
                ))
            })?;

            Ok(domain.to_string())
        } else if resource.starts_with("http:") || resource.starts_with("https:") {
            // For http(s): URIs, parse the URL and extract the host
            let url = Url::parse(resource)?;
            match url.host_str() {
                Some(host) => Ok(host.to_string()),
                None => Err(WebFingerError::HostExtractionFailed(format!(
                    "Cannot extract host from URI: {}",
                    resource
                ))),
            }
        } else {
            Err(WebFingerError::InvalidResource(format!(
                "Unsupported URI scheme: {}",
                resource
            )))
        }
    }
}

impl Default for WebFingerClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_extract_host_from_acct() {
        let client = WebFingerClient::new();

        assert_eq!(
            client.extract_host("acct:user@example.com").unwrap(),
            "example.com"
        );

        assert_eq!(
            client.extract_host("acct:user@example.com:8000").unwrap(),
            "example.com"
        );

        assert!(client.extract_host("acct:userexample.com").is_err());
    }

    #[tokio::test]
    async fn test_extract_host_from_http() {
        let client = WebFingerClient::new();

        assert_eq!(
            client.extract_host("https://example.com/path").unwrap(),
            "example.com"
        );

        assert_eq!(
            client
                .extract_host("http://example.org:8080/path?query=value")
                .unwrap(),
            "example.org"
        );

        assert!(client.extract_host("ftp://example.com").is_err());
    }

    #[tokio::test]
    async fn test_webfinger_request() {
        let mut server = Server::new_async().await;

        let mock_response = r#"{
            "subject": "acct:user@example.com",
            "aliases": [
                "https://example.com/users/user"
            ],
            "properties": {
                "http://example.com/ns/role": "administrator"
            },
            "links": [
                {
                    "rel": "http://webfinger.net/rel/profile-page",
                    "href": "https://example.com/users/user"
                },
                {
                    "rel": "self",
                    "href": "https://example.com/api/user"
                }
            ]
        }"#;

        // Setup the mock endpoint
        let m = server
            .mock("GET", "/.well-known/webfinger")
            .match_query(mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
                "resource".into(),
                "acct:user@example.com".into(),
            )]))
            .with_status(200)
            .with_header("content-type", "application/jrd+json")
            .with_body(mock_response)
            .create_async()
            .await;

        // Create a client that will connect to the mock server
        let client = WebFingerClient::new();

        // Override the extract_host method for test
        let resource = "acct:user@example.com";
        let host = server.host_with_port();

        // Construct the WebFinger URL
        let mut webfinger_url =
            Url::parse(&format!("http://{}/.well-known/webfinger", host)).unwrap();

        // Add query parameters
        let mut query_pairs = webfinger_url.query_pairs_mut();
        query_pairs.append_pair("resource", resource);
        drop(query_pairs);

        // Make the request directly to the mock server
        let response = client
            .client
            .get(webfinger_url)
            .header("Accept", "application/jrd+json, application/json")
            .send()
            .await
            .unwrap();

        let jrd = response.json::<JrdResource>().await.unwrap();

        // Verify the response
        assert_eq!(jrd.subject.clone().unwrap(), "acct:user@example.com");
        assert_eq!(
            jrd.aliases.clone().unwrap(),
            vec!["https://example.com/users/user"]
        );
        assert_eq!(jrd.links.as_ref().unwrap().len(), 2);

        let profile_link = jrd
            .find_link("http://webfinger.net/rel/profile-page")
            .unwrap();
        assert_eq!(
            profile_link.href.as_ref().unwrap(),
            "https://example.com/users/user"
        );

        m.assert_async().await;
    }
}
