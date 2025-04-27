//! Webfinger protocol implementation based on RFC 7033.
//!
//! This module implements the WebFinger protocol as specified in
//! RFC 7033 (https://datatracker.ietf.org/doc/html/rfc7033).
//! It provides functionality to serve webfinger resources from disk in JSON format.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use thiserror::Error;

/// WebFinger request parameters as defined in RFC 7033
#[derive(Debug, Deserialize)]
pub struct WebfingerQuery {
    /// The resource to query for (e.g. acct:user@example.com)
    pub resource: String,
    
    /// Optional requested relation types to filter the response
    #[serde(rename = "rel")]
    pub relations: Option<Vec<String>>,
}

/// WebFinger error types
#[derive(Debug, Error)]
pub enum WebfingerError {
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),
    
    #[error("Invalid resource format: {0}")]
    InvalidResource(String),
    
    #[error("File system error: {0}")]
    FileSystemError(#[from] std::io::Error),
    
    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl IntoResponse for WebfingerError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            WebfingerError::ResourceNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            WebfingerError::InvalidResource(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        
        (status, message).into_response()
    }
}

/// WebFinger Resource Descriptor as defined in RFC 7033 Section 4.4
#[derive(Debug, Serialize, Deserialize)]
pub struct JRD {
    /// Subject URI identifying the entity that the JRD describes
    pub subject: String,
    
    /// List of aliases for the subject
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,
    
    /// List of properties associated with the JRD
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
    
    /// List of links associated with the JRD
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<Link>>,
}

/// Link structure as defined in RFC 7033 Section 4.4.4
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Link {
    /// The relation type of the link
    pub rel: String,
    
    /// The link type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    
    /// The HTTP reference of the link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    
    /// List of link template URIs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titles: Option<serde_json::Map<String, serde_json::Value>>,
    
    /// List of properties associated with the link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Handles webfinger requests and serves responses from disk
async fn handle_webfinger(
    Query(query): Query<WebfingerQuery>,
    State(webfinger_dir): State<PathBuf>,
) -> Result<Json<JRD>, WebfingerError> {
    // Validate the resource format
    if !query.resource.starts_with("acct:") && !query.resource.starts_with("https://") {
        return Err(WebfingerError::InvalidResource(format!(
            "Resource must start with 'acct:' or 'https://': {}",
            query.resource
        )));
    }
    
    // Extract identifier from the resource URI
    // For acct:user@example.com or https://example.com/user, we want "user@example.com" or "user"
    let identifier = if query.resource.starts_with("acct:") {
        query.resource.strip_prefix("acct:").map(|s| s.to_string()).unwrap()
    } else {
        // For https URLs, extract the last path component
        let url = url::Url::parse(&query.resource)
            .map_err(|_| WebfingerError::InvalidResource(query.resource.clone()))?;
        
        url.path_segments()
            .and_then(|segments| segments.last())
            .map(|segment| segment.to_string())
            .ok_or_else(|| WebfingerError::InvalidResource(query.resource.clone()))?
    };
    
    // Construct the file path for the webfinger JSON
    let file_path = webfinger_dir.join(format!("{}.json", identifier));
    
    // Check if the file exists
    if !file_path.exists() {
        return Err(WebfingerError::ResourceNotFound(query.resource));
    }
    
    // Read and parse the JSON file
    let file_content = fs::read_to_string(file_path)?;
    let mut jrd: JRD = serde_json::from_str(&file_content)?;
    
    // Filter relations if requested
    if let Some(relations) = &query.relations {
        if let Some(links) = &mut jrd.links {
            jrd.links = Some(
                links
                    .iter()
                    .filter(|link| relations.contains(&link.rel))
                    .cloned()
                    .collect(),
            );
        }
    }
    
    Ok(Json(jrd))
}

/// Creates a router for webfinger endpoints
pub fn webfinger_router(webfinger_dir: impl Into<PathBuf>) -> Router {
    let webfinger_path = webfinger_dir.into();
    
    Router::new()
        .route(
            "/.well-known/webfinger",
            get(handle_webfinger)
        )
        .with_state(webfinger_path)
}
