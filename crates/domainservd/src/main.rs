//! Domain Service Daemon
//!
//! This service is responsible for handling domain-specific operations,
//! including webfinger protocol implementation, according to RFC 7033.

mod activitypub;
mod db;
mod delivery;
mod rabbitmq;
mod webfinger;

use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use db::MongoDB;
use oxifed::database::DatabaseManager;
use oxifed::pki::PkiManager;
use std::io;
use std::sync::Arc;
use thiserror::Error;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// MongoDB connection
    pub db: Arc<MongoDB>,
    /// LavinMQ connection pool
    pub mq_pool: deadpool_lapin::Pool,
    /// Database manager for ActivityPub operations
    pub db_manager: Arc<DatabaseManager>,
    /// PKI manager for cryptographic operations
    pub pki_manager: Arc<PkiManager>,
    /// Admin API URL advertised via domain-level WebFinger
    pub admin_api_url: Option<String>,
    /// OIDC issuer URL advertised via domain-level WebFinger
    pub oidc_issuer_url: Option<String>,
    /// OIDC audience the admin API expects in tokens
    pub oidc_audience: Option<String>,
}

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

    /// Database error
    #[error("Database error: {0}")]
    DbError(#[from] db::DbError),

    /// RabbitMQ/LavinMQ error
    #[error("RabbitMQ error: {0}")]
    RabbitMQError(#[from] rabbitmq::RabbitMQError),

    /// External database error
    #[error("External database error: {0}")]
    DatabaseError(#[from] oxifed::database::DatabaseError),
}

/// Extract domain from Host header
pub fn extract_domain_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("host")
        .and_then(|host| host.to_str().ok())
        .map(|host| {
            // Remove port if present
            host.split(':').next().unwrap_or(host).to_string()
        })
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

#[tokio::main]
async fn main() -> Result<(), DomainservdError> {
    // Configure logging
    tracing_subscriber::fmt::init();

    // Initialize MongoDB connection
    let mongo_uri = std::env::var("MONGODB_URI")
        .unwrap_or_else(|_| "mongodb://root:password@localhost:27017".to_string());
    let db_name = std::env::var("MONGODB_DBNAME").unwrap_or_else(|_| "domainservd".to_string());

    tracing::info!("Connecting to MongoDB at {}", mongo_uri);
    let mongodb = MongoDB::new(&mongo_uri, &db_name).await?;

    // Initialize collections
    mongodb.init_collections().await?;
    tracing::info!("MongoDB initialized successfully");

    // Share MongoDB connection across handlers
    let db = Arc::new(mongodb);

    // Initialize LavinMQ connection
    let amqp_url = std::env::var("AMQP_URI")
        .or_else(|_| std::env::var("AMQP_URL"))
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());

    tracing::info!("Connecting to LavinMQ at {}", amqp_url);
    let mq_pool = rabbitmq::create_connection_pool(&amqp_url);

    // Initialize RabbitMQ exchanges and queues
    rabbitmq::init_rabbitmq(&mq_pool).await?;
    tracing::info!("LavinMQ initialized successfully");

    // Create database manager
    let db_manager = Arc::new(DatabaseManager::new(db.database().clone()));
    db_manager.initialize().await?;

    // Create PKI manager (in a real implementation, this would load existing keys)
    let pki_manager = Arc::new(PkiManager::new());

    // Read optional discovery URLs for domain-level WebFinger
    let admin_api_url = std::env::var("ADMIN_API_URL").ok();
    let oidc_issuer_url = std::env::var("OIDC_ISSUER_URL").ok();
    let oidc_audience = std::env::var("OIDC_AUDIENCE").ok();

    // Create an application state
    let app_state = AppState {
        db: db.clone(),
        mq_pool: mq_pool.clone(),
        db_manager: db_manager.clone(),
        pki_manager: pki_manager.clone(),
        admin_api_url,
        oidc_issuer_url,
        oidc_audience,
    };

    // Start message consumer in a separate task
    rabbitmq::start_consumers(mq_pool, db.clone()).await?;

    let app = Router::new()
        .route("/health", get(health_check))
        .merge(webfinger::webfinger_router(app_state.clone()))
        .merge(activitypub::activitypub_router(app_state.clone()))
        .with_state(app_state);

    let addr = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
