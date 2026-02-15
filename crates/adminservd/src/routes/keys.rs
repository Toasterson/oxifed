use axum::Json;
use axum::extract::State;
use oxifed::messaging::KeyGenerateMessage;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::AuthenticatedUser;
use crate::error::ApiError;
use crate::messaging;

#[derive(Deserialize)]
pub struct KeyGenerateRequest {
    pub actor: String,
    pub algorithm: String,
    pub key_size: Option<u32>,
}

pub async fn generate_key(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(body): Json<KeyGenerateRequest>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    let message = KeyGenerateMessage::new(body.actor, body.algorithm, body.key_size);
    messaging::publish_message(&state.mq_pool, &message)
        .await
        .map_err(ApiError::from)?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(json!({"status": "queued"})),
    ))
}
