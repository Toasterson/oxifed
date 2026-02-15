use axum::Json;
use axum::extract::{Path, Query, State};
use oxifed::messaging::{ProfileCreateMessage, ProfileDeleteMessage, ProfileUpdateMessage};
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

pub async fn create_person(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<ProfileCreateMessage>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    messaging::publish_message(&state.mq_pool, &body)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}

pub async fn update_person(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
    Json(mut body): Json<ProfileUpdateMessage>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    body.subject = id;
    messaging::publish_message(&state.mq_pool, &body)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}

pub async fn delete_person(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<String>,
    Query(query): Query<DeleteQuery>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    let message = ProfileDeleteMessage::new(id, query.force);
    messaging::publish_message(&state.mq_pool, &message)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}
