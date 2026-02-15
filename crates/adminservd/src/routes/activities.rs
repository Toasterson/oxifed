use axum::Json;
use axum::extract::State;
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
