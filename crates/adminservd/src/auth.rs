use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Instant;

use crate::AppState;
use crate::error::ApiError;

/// OIDC provider configuration
#[derive(Clone)]
pub struct OidcConfig {
    pub issuer_url: String,
    pub audience: String,
    pub jwks_uri: String,
}

/// Cached JWKS keys
pub struct JwksCache {
    pub keys: HashMap<String, DecodingKey>,
    pub jwks_uri: String,
    pub fetched_at: Instant,
}

impl JwksCache {
    pub fn new(jwks_uri: String) -> Self {
        Self {
            keys: HashMap::new(),
            jwks_uri,
            fetched_at: Instant::now(),
        }
    }

    pub fn is_stale(&self) -> bool {
        self.fetched_at.elapsed() > std::time::Duration::from_secs(300) // 5 minutes
    }
}

/// Discover OIDC metadata and extract jwks_uri
pub async fn discover_oidc(issuer_url: &str) -> Result<String, ApiError> {
    let well_known = format!(
        "{}/.well-known/openid-configuration",
        issuer_url.trim_end_matches('/')
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&well_known)
        .send()
        .await
        .map_err(|e| ApiError::OidcError(format!("Failed to fetch OIDC metadata: {}", e)))?;

    let metadata: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ApiError::OidcError(format!("Failed to parse OIDC metadata: {}", e)))?;

    metadata["jwks_uri"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::OidcError("OIDC metadata missing jwks_uri".into()))
}

/// Fetch JWKS from the provider and populate the cache
pub async fn fetch_jwks(jwks_cache: &Arc<RwLock<JwksCache>>) -> Result<(), ApiError> {
    let jwks_uri = {
        let cache = jwks_cache.read().await;
        cache.jwks_uri.clone()
    };

    let client = reqwest::Client::new();
    let response = client
        .get(&jwks_uri)
        .send()
        .await
        .map_err(|e| ApiError::OidcError(format!("Failed to fetch JWKS: {}", e)))?;

    let jwk_set: JwkSet = response
        .json()
        .await
        .map_err(|e| ApiError::OidcError(format!("Failed to parse JWKS: {}", e)))?;

    let mut keys = HashMap::new();
    for jwk in &jwk_set.keys {
        if let Some(kid) = &jwk.common.key_id
            && let Ok(decoding_key) = DecodingKey::from_jwk(jwk)
        {
            keys.insert(kid.clone(), decoding_key);
        }
    }

    let mut cache = jwks_cache.write().await;
    cache.keys = keys;
    cache.fetched_at = Instant::now();

    Ok(())
}

/// JWT claims
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub aud: ClaimAudience,
    pub exp: usize,
    #[serde(default)]
    pub iat: Option<usize>,
}

/// Audience can be a single string or array of strings
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum ClaimAudience {
    Single(String),
    Multiple(Vec<String>),
}

/// Authenticated user extracted from a valid JWT
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AuthenticatedUser {
    pub sub: String,
}

impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // Extract Authorization header
        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(ApiError::InvalidToken("Expected Bearer token".to_string()))?;

        // Decode header to get kid
        let header = decode_header(token)
            .map_err(|e| ApiError::InvalidToken(format!("Invalid token header: {}", e)))?;

        let kid = header
            .kid
            .ok_or(ApiError::InvalidToken("Token missing kid".to_string()))?;

        // Try to get the decoding key, refreshing cache if needed
        let decoding_key = {
            let cache = state.jwks_cache.read().await;
            cache.keys.get(&kid).cloned()
        };

        let decoding_key = match decoding_key {
            Some(key) => key,
            None => {
                // Key not found — refresh JWKS cache
                fetch_jwks(&state.jwks_cache).await?;
                let cache = state.jwks_cache.read().await;
                cache
                    .keys
                    .get(&kid)
                    .cloned()
                    .ok_or(ApiError::InvalidToken(format!(
                        "Unknown signing key: {}",
                        kid
                    )))?
            }
        };

        // Also refresh if stale
        {
            let cache = state.jwks_cache.read().await;
            if cache.is_stale() {
                drop(cache);
                let _ = fetch_jwks(&state.jwks_cache).await;
            }
        }

        // Validate the token
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&state.oidc_config.issuer_url]);
        validation.set_audience(&[&state.oidc_config.audience]);
        validation.set_required_spec_claims(&["exp", "sub", "iss", "aud"]);
        validation.leeway = 60;

        let token_data = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|e| ApiError::InvalidToken(format!("Token validation failed: {}", e)))?;

        Ok(AuthenticatedUser {
            sub: token_data.claims.sub,
        })
    }
}
