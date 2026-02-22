//! OIDC Device Code Grant flow for oxiadm
//!
//! Implements the OAuth 2.0 Device Authorization Grant (RFC 8628) using
//! plain HTTP requests via reqwest. Discovers endpoints from OIDC metadata.

use chrono::Utc;
use miette::{IntoDiagnostic, Result, miette};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::context::{self, ServerConfig};

/// OIDC discovery document (subset of fields we need)
#[derive(Deserialize)]
struct OidcMetadata {
    token_endpoint: String,
    device_authorization_endpoint: Option<String>,
}

/// Device authorization response
#[derive(Deserialize)]
struct DeviceAuthResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    #[serde(default = "default_interval")]
    interval: u64,
    expires_in: u64,
    /// Returned by providers that support client auto-registration
    client_id: Option<String>,
    /// Returned by providers that support client auto-registration
    client_secret: Option<String>,
}

fn default_interval() -> u64 {
    5
}

/// Token response
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

/// Token error response
#[derive(Deserialize)]
struct TokenErrorResponse {
    error: String,
    error_description: Option<String>,
}

/// Token poll request body
#[derive(Serialize)]
struct DeviceTokenRequest<'a> {
    grant_type: &'a str,
    device_code: &'a str,
    client_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_secret: Option<&'a str>,
}

/// Refresh token request body
#[derive(Serialize)]
struct RefreshTokenRequest<'a> {
    grant_type: &'a str,
    refresh_token: &'a str,
    client_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_secret: Option<&'a str>,
}

/// Discover OIDC metadata from the issuer URL
async fn discover_metadata(client: &reqwest::Client, issuer_url: &str) -> Result<OidcMetadata> {
    let well_known = format!(
        "{}/.well-known/openid-configuration",
        issuer_url.trim_end_matches('/')
    );

    let response = client
        .get(&well_known)
        .send()
        .await
        .into_diagnostic()
        .map_err(|e| miette!("Failed to fetch OIDC metadata: {}", e))?;

    if !response.status().is_success() {
        return Err(miette!(
            help = "Check that the issuer URL is correct and the OIDC provider is reachable",
            "OIDC discovery failed with status {}",
            response.status()
        ));
    }

    response
        .json::<OidcMetadata>()
        .await
        .into_diagnostic()
        .map_err(|e| miette!("Failed to parse OIDC metadata: {}", e))
}

/// Run the Device Code Grant flow and return the resulting auth fields.
///
/// This is the core flow used by both `device_code_login` (for the current server)
/// and `device_code_login_for_server` (for add-server).
async fn run_device_code_flow(
    issuer_url: &str,
    client_id: Option<&str>,
    audience: Option<&str>,
) -> Result<(TokenResponse, String, Option<String>)> {
    let client = reqwest::Client::new();

    // Discover OIDC endpoints
    let metadata = discover_metadata(&client, issuer_url).await?;

    let device_auth_endpoint = metadata.device_authorization_endpoint.ok_or_else(|| {
        miette!(
            help = "The OIDC provider may not support the Device Authorization Grant. \
                    Check with your identity provider.",
            "OIDC provider does not advertise a device_authorization_endpoint"
        )
    })?;

    // Request device authorization — with or without client_id
    let mut form_params = vec![("scope", "openid profile offline_access"), ("client_name", "oxiadm")];
    if let Some(id) = client_id {
        form_params.push(("client_id", id));
    }
    if let Some(aud) = audience {
        form_params.push(("audience", aud));
    }

    let response = client
        .post(&device_auth_endpoint)
        .form(&form_params)
        .send()
        .await
        .into_diagnostic()
        .map_err(|e| miette!("Device authorization request failed: {}", e))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(miette!("Device authorization request failed: {}", body));
    }

    let device_auth: DeviceAuthResponse = response
        .json()
        .await
        .into_diagnostic()
        .map_err(|e| miette!("Failed to parse device authorization response: {}", e))?;

    // Resolve the client_id to use for token polling
    let effective_client_id = match client_id {
        Some(id) => id.to_string(),
        None => device_auth.client_id.clone().ok_or_else(|| {
            miette!(
                help = "Try again with an explicit --client-id",
                "Provider did not return a client_id from auto-registration"
            )
        })?,
    };
    let effective_client_secret = device_auth.client_secret.as_deref();

    // Display instructions to the user
    println!();
    println!("To authenticate, visit: {}", device_auth.verification_uri);
    if let Some(ref complete_uri) = device_auth.verification_uri_complete {
        println!("Or open: {}", complete_uri);
    }
    println!("Enter code: {}", device_auth.user_code);
    println!();
    println!("Waiting for authorization...");

    // Poll for the token
    let poll_interval = Duration::from_secs(device_auth.interval);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(device_auth.expires_in);

    let token_response = loop {
        tokio::time::sleep(poll_interval).await;

        if tokio::time::Instant::now() > deadline {
            return Err(miette!(
                help = "Try again",
                "Device authorization timed out"
            ));
        }

        let response = client
            .post(&metadata.token_endpoint)
            .form(&DeviceTokenRequest {
                grant_type: "urn:ietf:params:oauth:grant-type:device_code",
                device_code: &device_auth.device_code,
                client_id: &effective_client_id,
                client_secret: effective_client_secret,
            })
            .send()
            .await
            .into_diagnostic()
            .map_err(|e| miette!("Token poll request failed: {}", e))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .into_diagnostic()
            .map_err(|e| miette!("Failed to read token response: {}", e))?;

        if status.is_success() {
            let token: TokenResponse = serde_json::from_str(&body)
                .into_diagnostic()
                .map_err(|e| miette!("Failed to parse token response: {}", e))?;
            break token;
        }

        // Check if we should keep polling
        if let Ok(error) = serde_json::from_str::<TokenErrorResponse>(&body) {
            match error.error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
                "expired_token" => {
                    return Err(miette!(
                        help = "Try again",
                        "Device code expired before authorization was completed"
                    ));
                }
                "access_denied" => {
                    return Err(miette!("Authorization was denied by the user"));
                }
                _ => {
                    let desc = error.error_description.unwrap_or_default();
                    return Err(miette!("Token exchange failed: {} {}", error.error, desc));
                }
            }
        } else {
            return Err(miette!(
                "Unexpected token endpoint response ({}): {}",
                status,
                body
            ));
        }
    };

    Ok((
        token_response,
        effective_client_id,
        device_auth.client_secret,
    ))
}

/// Perform Device Code Grant login for the current server.
pub async fn device_code_login(issuer_url: Option<&str>, client_id: Option<&str>) -> Result<()> {
    let mut ctx = context::load_context()?;
    let server = ctx.current_server_mut()?;

    let effective_issuer = match issuer_url {
        Some(url) => url.to_string(),
        None => server.issuer_url.clone().ok_or_else(|| {
            miette!(
                help = "Provide --issuer-url or add the server with: oxiadm add-server <hostname>",
                "No OIDC issuer URL configured for server '{}'",
                server.hostname
            )
        })?,
    };

    let audience = server.audience.as_deref();

    let (token_response, effective_client_id, auto_client_secret) =
        run_device_code_flow(&effective_issuer, client_id, audience).await?;

    let expires_at = token_response.expires_in.map(|secs| {
        let expiry = Utc::now() + chrono::Duration::seconds(secs as i64);
        expiry.to_rfc3339()
    });

    // Re-load to avoid stale data
    let mut ctx = context::load_context()?;
    let server = ctx.current_server_mut()?;
    server.issuer_url = Some(effective_issuer);
    server.client_id = Some(effective_client_id);
    server.client_secret = auto_client_secret;
    server.access_token = Some(token_response.access_token);
    server.refresh_token = token_response.refresh_token;
    server.expires_at = expires_at;
    context::save_context(&ctx)?;

    println!("Login successful!");
    Ok(())
}

/// Perform Device Code Grant login for a specific server (used by `add-server`).
///
/// Creates or updates the server entry in context and sets it as current.
pub async fn device_code_login_for_server(
    hostname: &str,
    admin_api_url: &str,
    issuer_url: &str,
    client_id: Option<&str>,
    audience: Option<&str>,
) -> Result<()> {
    let (token_response, effective_client_id, auto_client_secret) =
        run_device_code_flow(issuer_url, client_id, audience).await?;

    let expires_at = token_response.expires_in.map(|secs| {
        let expiry = Utc::now() + chrono::Duration::seconds(secs as i64);
        expiry.to_rfc3339()
    });

    let mut ctx = context::load_context()?;

    let server_config = ServerConfig {
        hostname: hostname.to_string(),
        admin_api_url: admin_api_url.to_string(),
        issuer_url: Some(issuer_url.to_string()),
        audience: audience.map(|s| s.to_string()),
        client_id: Some(effective_client_id),
        client_secret: auto_client_secret,
        access_token: Some(token_response.access_token),
        refresh_token: token_response.refresh_token,
        expires_at,
    };

    // Replace existing or add new
    if let Some(existing) = ctx.find_server_mut(hostname) {
        *existing = server_config;
    } else {
        ctx.servers.push(server_config);
    }

    ctx.context.current_server = Some(hostname.to_string());
    context::save_context(&ctx)?;

    println!("Login successful!");
    Ok(())
}

/// Refresh the access token if it's expired or about to expire (for the current server).
pub async fn refresh_token_if_needed() -> Result<()> {
    let ctx = context::load_context()?;
    let server = ctx.current_server()?;

    // Check if we have a token at all
    if server.access_token.is_none() {
        return Err(miette!(
            help = "Log in first with: oxiadm login",
            "No access token found for server '{}' — you are not logged in",
            server.hostname
        ));
    }

    // Check if token is expired
    let needs_refresh = if let Some(ref expires_at) = server.expires_at {
        match chrono::DateTime::parse_from_rfc3339(expires_at) {
            Ok(expiry) => {
                // Refresh if token expires within 60 seconds
                Utc::now() + chrono::Duration::seconds(60) > expiry
            }
            Err(_) => true,
        }
    } else {
        false // No expiry info, assume it's valid
    };

    if !needs_refresh {
        return Ok(());
    }

    // Need to refresh
    let refresh_token = server.refresh_token.as_ref().ok_or_else(|| {
        miette!(
            help = "Log in again with: oxiadm login",
            "Token expired and no refresh token available"
        )
    })?;

    let issuer_url = server.issuer_url.as_ref().ok_or_else(|| {
        miette!(
            help = "Log in again with: oxiadm login",
            "No issuer URL stored — cannot refresh token"
        )
    })?;

    let client_id = server.client_id.as_ref().ok_or_else(|| {
        miette!(
            help = "Log in again with: oxiadm login",
            "No client ID stored — cannot refresh token"
        )
    })?;

    let client_secret = server.client_secret.as_deref();

    let http_client = reqwest::Client::new();
    let metadata = discover_metadata(&http_client, issuer_url).await?;

    let response = http_client
        .post(&metadata.token_endpoint)
        .form(&RefreshTokenRequest {
            grant_type: "refresh_token",
            refresh_token,
            client_id,
            client_secret,
        })
        .send()
        .await
        .into_diagnostic()
        .map_err(|e| miette!("Token refresh request failed: {}", e))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(miette!(
            help = "Log in again with: oxiadm login",
            "Token refresh failed: {}",
            body
        ));
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .into_diagnostic()
        .map_err(|e| miette!("Failed to parse token refresh response: {}", e))?;

    let expires_at = token_response.expires_in.map(|secs| {
        let expiry = Utc::now() + chrono::Duration::seconds(secs as i64);
        expiry.to_rfc3339()
    });

    let mut ctx = context::load_context()?;
    let server = ctx.current_server_mut()?;
    server.access_token = Some(token_response.access_token);
    if let Some(new_refresh) = token_response.refresh_token {
        server.refresh_token = Some(new_refresh);
    }
    server.expires_at = expires_at;
    context::save_context(&ctx)?;

    tracing::debug!("Token refreshed successfully");
    Ok(())
}

/// Clear auth tokens for the current server.
pub fn logout() -> Result<()> {
    let mut ctx = context::load_context()?;
    let server = ctx.current_server_mut()?;
    server.access_token = None;
    server.refresh_token = None;
    server.expires_at = None;
    server.client_id = None;
    server.client_secret = None;
    let hostname = server.hostname.clone();
    context::save_context(&ctx)?;
    println!("Logged out from server '{}'", hostname);
    Ok(())
}
