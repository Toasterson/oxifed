//! Webfinger protocol implementation based on RFC 7033.
//!
//! This module implements the WebFinger protocol as specified in
//! RFC 7033 (https://datatracker.ietf.org/doc/html/rfc7033).
//! It provides functionality to serve webfinger resources from disk in JSON format.

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use mongodb::bson::doc;
use oxifed::webfinger::JrdResource;
use serde::Deserialize;
use thiserror::Error;
use tracing::debug;

use crate::AppState;

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

    #[error("Database error: {0}")]
    DbError(#[from] mongodb::error::Error),

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

/// Handles webfinger requests and serves responses from MongoDB
async fn handle_webfinger(
    Query(query): Query<WebfingerQuery>,
    State(state): State<AppState>,
) -> Result<Json<JrdResource>, WebfingerError> {
    // Validate the resource format
    if !query.resource.starts_with("acct:")
        && !query.resource.starts_with("act:")
        && !query.resource.starts_with("https://")
    {
        debug!(
            "Tried to fetch webfinger resource with invalid format: {}",
            query.resource
        );
        return Err(WebfingerError::InvalidResource(format!(
            "Resource must start with 'acct:' or 'https://': {}",
            query.resource
        )));
    }

    // Use the full resource as the subject for lookup
    let subject = query.resource.replace("act:", "acct:").clone();

    // Query MongoDB for the JrdResource
    let profiles_collection = state.db.webfinger_profiles_collection();
    let filter = doc! { "subject": subject.clone() };

    // Attempt to find the resource in MongoDB
    let jrd_result = profiles_collection.find_one(filter).await?;

    // Return 404 if not found
    let mut jrd = jrd_result.ok_or_else(|| {
        debug!("Webfinger resource not found in database: {}", subject);
        WebfingerError::ResourceNotFound(subject)
    })?;

    // Filter relations if requested
    if let Some(relations) = &query.relations
        && let Some(links) = &mut jrd.links
    {
        jrd.links = Some(
            links
                .iter()
                .filter(|link| relations.contains(&link.rel))
                .cloned()
                .collect(),
        );
    }

    Ok(Json(jrd))
}

/// Creates a router for webfinger endpoints
pub fn webfinger_router(_state: AppState) -> Router<AppState> {
    Router::new().route("/.well-known/webfinger", get(handle_webfinger))
}
