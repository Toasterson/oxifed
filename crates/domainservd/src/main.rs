//! Domain Service Daemon
//!
//! This service is responsible for handling domain-specific operations,
//! including webfinger protocol implementation, according to RFC 7033.

mod webfinger;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use std::path::{absolute, PathBuf};
use thiserror::Error;
use std::io;

/// Errors that can occur in the domainservd service
#[derive(Error, Debug)]
pub enum DomainservdError {
    /// Error resolving an absolute path
    #[error("io error: {0}")]
    IOError(#[from] io::Error),
    
    /// Error with Axum web server
    #[error("Server error: {0}")]
    ServerError(#[from] axum::Error),
    
    /// Environment variable error
    #[error("Environment variable error: {0}")]
    EnvVarError(#[from] std::env::VarError),
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

#[tokio::main]
async fn main() -> Result<(), DomainservdError> {
    // Configure logging
    tracing_subscriber::fmt::init();

    let webfinger_dir =
        std::env::var("WEBFINGER_DIR").unwrap_or_else(|_| "./webfinger".to_string());
    let webfinger_path = absolute(PathBuf::from(webfinger_dir))?;

    // Ensure the webfinger directory exists
    if !webfinger_path.exists() {
        std::fs::create_dir_all(&webfinger_path)?;
        tracing::info!("Created webfinger directory: {:?}", webfinger_path);
    }

    let app = Router::new()
        .route("/health", get(health_check))
        .merge(webfinger::webfinger_router(webfinger_path));

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Listening on {}", addr);

    axum::serve(listener, app).await?;
    
    Ok(())
}
