mod auth;
mod error;
mod messaging;
mod routes;

use auth::{JwksCache, OidcConfig};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub mq_pool: deadpool_lapin::Pool,
    pub jwks_cache: Arc<RwLock<JwksCache>>,
    pub oidc_config: OidcConfig,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Read configuration from environment
    let amqp_url = std::env::var("AMQP_URI")
        .or_else(|_| std::env::var("AMQP_URL"))
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());

    let oidc_issuer_url =
        std::env::var("OIDC_ISSUER_URL").expect("OIDC_ISSUER_URL environment variable is required");

    let oidc_audience =
        std::env::var("OIDC_AUDIENCE").unwrap_or_else(|_| "oxifed-admin".to_string());

    let bind_address = std::env::var("BIND_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8081".to_string());

    // Create LavinMQ connection pool
    tracing::info!("Connecting to LavinMQ at {}", amqp_url);
    let config = deadpool_lapin::Config {
        url: Some(amqp_url),
        ..Default::default()
    };
    let mq_pool = config
        .create_pool(Some(deadpool_lapin::Runtime::Tokio1))
        .expect("Failed to create LavinMQ connection pool");

    // Initialize AMQP exchanges
    messaging::init_exchanges(&mq_pool).await?;
    tracing::info!("LavinMQ exchanges initialized");

    // Discover OIDC provider metadata
    tracing::info!("Discovering OIDC metadata from {}", oidc_issuer_url);
    let jwks_uri = auth::discover_oidc(&oidc_issuer_url).await?;
    tracing::info!("JWKS URI: {}", jwks_uri);

    let oidc_config = OidcConfig {
        issuer_url: oidc_issuer_url,
        audience: oidc_audience,
        jwks_uri: jwks_uri.clone(),
    };

    // Fetch initial JWKS
    let jwks_cache = Arc::new(RwLock::new(JwksCache::new(jwks_uri)));
    auth::fetch_jwks(&jwks_cache).await?;
    tracing::info!("JWKS cache populated");

    let app_state = AppState {
        mq_pool,
        jwks_cache,
        oidc_config,
    };

    // Build the router
    let app = routes::api_router()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_address).await?;
    tracing::info!("adminservd listening on {}", bind_address);

    axum::serve(listener, app).await?;

    Ok(())
}
