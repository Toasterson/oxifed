//! Domain Service Daemon
//!
//! This service is responsible for handling domain-specific operations,
//! including webfinger protocol implementation, according to RFC 7033.

mod webfinger;
mod db;
mod rabbitmq;

use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use deadpool_lapin::Pool as RabbitPool;
use std::sync::Arc;
use thiserror::Error;
use std::io;
use db::MongoDB;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// MongoDB connection
    pub db: Arc<MongoDB>,
    /// LavinMQ connection pool
    pub mq_pool: RabbitPool,
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
    let db_name = std::env::var("MONGODB_DBNAME")
        .unwrap_or_else(|_| "domainservd".to_string());
    
    tracing::info!("Connecting to MongoDB at {}", mongo_uri);
    let mongodb = MongoDB::new(&mongo_uri, &db_name).await?;
    
    // Initialize collections
    mongodb.init_collections().await?;
    tracing::info!("MongoDB initialized successfully");
    
    // Share MongoDB connection across handlers
    let db = Arc::new(mongodb);
    
    // Initialize LavinMQ connection
    let amqp_url = std::env::var("AMQP_URL")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());
    
    tracing::info!("Connecting to LavinMQ at {}", amqp_url);
    let mq_pool = rabbitmq::create_connection_pool(&amqp_url);
    
    // Initialize RabbitMQ exchanges and queues
    rabbitmq::init_rabbitmq(&mq_pool).await?;
    tracing::info!("LavinMQ initialized successfully");
    
    // Create application state
    let app_state = AppState {
        db: db.clone(),
        mq_pool: mq_pool.clone(),
    };
    
    // Start message consumer in a separate task
    rabbitmq::start_consumer(mq_pool, db.clone()).await?;
    
    let app = Router::new()
        .route("/health", get(health_check))
        .merge(webfinger::webfinger_router(app_state.clone()))
        .with_state(app_state);

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Listening on {}", addr);

    axum::serve(listener, app).await?;
    
    Ok(())
}
