use axum::Json;
use axum::extract::{Path, Query, State};
use oxifed::messaging::{DomainCreateMessage, DomainDeleteMessage, DomainUpdateMessage};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::error::ApiError;
use crate::messaging;

#[derive(Deserialize)]
pub struct DeleteQuery {
    #[serde(default)]
    pub force: bool,
}

pub async fn list_domains(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<Json<Value>, ApiError> {
    let domains = messaging::list_domains(&state.mq_pool)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(serde_json::to_value(domains).map_err(|e| {
        ApiError::Internal(format!("Serialization error: {}", e))
    })?))
}

pub async fn create_domain(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<DomainCreateMessage>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    messaging::publish_message(&state.mq_pool, &body)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}

pub async fn get_domain(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(name): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let domain = messaging::get_domain(&state.mq_pool, &name)
        .await
        .map_err(ApiError::from)?;

    match domain {
        Some(d) => Ok(Json(serde_json::to_value(d).map_err(|e| {
            ApiError::Internal(format!("Serialization error: {}", e))
        })?)),
        None => Err(ApiError::NotFound(format!("Domain '{}' not found", name))),
    }
}

pub async fn update_domain(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(name): Path<String>,
    Json(mut body): Json<DomainUpdateMessage>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    // Ensure the domain name in the path matches the body
    body.domain = name;
    messaging::publish_message(&state.mq_pool, &body)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}

pub async fn delete_domain(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(name): Path<String>,
    Query(query): Query<DeleteQuery>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    let message = DomainDeleteMessage::new(name, query.force);
    messaging::publish_message(&state.mq_pool, &message)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}
