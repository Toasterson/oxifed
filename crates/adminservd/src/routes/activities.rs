use axum::Json;
use axum::extract::{Query, State};
use oxifed::messaging::{AnnounceActivityMessage, FollowActivityMessage, LikeActivityMessage};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::error::ApiError;
use crate::messaging;

#[derive(Deserialize)]
pub struct FollowRequest {
    pub actor: String,
    pub object: String,
}

#[derive(Deserialize)]
pub struct LikeRequest {
    pub actor: String,
    pub object: String,
}

#[derive(Deserialize)]
pub struct AnnounceRequest {
    pub actor: String,
    pub object: String,
    pub to: Option<String>,
    pub cc: Option<String>,
}

pub async fn follow(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<FollowRequest>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    let message = FollowActivityMessage::new(body.actor, body.object);
    messaging::publish_message(&state.mq_pool, &message)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}

pub async fn like(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<LikeRequest>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    let message = LikeActivityMessage::new(body.actor, body.object);
    messaging::publish_message(&state.mq_pool, &message)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}

pub async fn announce(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<AnnounceRequest>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    let message = AnnounceActivityMessage::new(body.actor, body.object, body.to, body.cc);
    messaging::publish_message(&state.mq_pool, &message)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}

#[derive(Deserialize)]
pub struct FollowsQuery {
    pub actor: String,
}

pub async fn list_following(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Query(query): Query<FollowsQuery>,
) -> Result<Json<Value>, ApiError> {
    let follows = messaging::list_following(&state.mq_pool, &query.actor)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(serde_json::to_value(follows).map_err(|e| {
        ApiError::Internal(format!("Serialization error: {}", e))
    })?))
}

pub async fn list_followers(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Query(query): Query<FollowsQuery>,
) -> Result<Json<Value>, ApiError> {
    let follows = messaging::list_followers(&state.mq_pool, &query.actor)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(serde_json::to_value(follows).map_err(|e| {
        ApiError::Internal(format!("Serialization error: {}", e))
    })?))
}
