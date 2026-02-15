use axum::Json;
use axum::extract::{Path, State};
use oxifed::messaging::UserCreateMessage;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::error::ApiError;
use crate::messaging;

pub async fn list_users(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<Json<Value>, ApiError> {
    let users = messaging::list_users(&state.mq_pool)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(serde_json::to_value(users).map_err(|e| {
        ApiError::Internal(format!("Serialization error: {}", e))
    })?))
}

pub async fn create_user(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<UserCreateMessage>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    messaging::publish_message(&state.mq_pool, &body)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}

pub async fn get_user(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(username): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let user = messaging::get_user(&state.mq_pool, &username)
        .await
        .map_err(ApiError::from)?;

    match user {
        Some(u) => Ok(Json(serde_json::to_value(u).map_err(|e| {
            ApiError::Internal(format!("Serialization error: {}", e))
        })?)),
        None => Err(ApiError::NotFound(format!("User '{}' not found", username))),
    }
}
