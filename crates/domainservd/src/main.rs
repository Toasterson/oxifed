//! Domain Service Daemon
//!
//! This service is responsible for handling domain-specific operations,
//! including webfinger protocol implementation according to RFC 7033.

mod webfinger;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use std::path::{absolute, PathBuf};

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

#[tokio::main]
async fn main() {
    // Configure logging
    tracing_subscriber::fmt::init();

    let webfinger_dir =
        std::env::var("WEBFINGER_DIR").unwrap_or_else(|_| "./webfinger".to_string());
    let webfinger_path = absolute(PathBuf::from(webfinger_dir)).unwrap();

    // Ensure the webfinger directory exists
    if !webfinger_path.exists() {
        std::fs::create_dir_all(&webfinger_path).expect("Failed to create webfinger directory");
        tracing::info!("Created webfinger directory: {:?}", webfinger_path);
    }

    let app = Router::new()
        .route("/health", get(health_check))
        .merge(webfinger::webfinger_router(webfinger_path));

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("Listening on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
