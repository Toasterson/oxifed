//! ActivityPub Publisher Daemon
//!
//! This daemon is responsible for processing activities from the message queue
//! and delivering them to followers according to the ActivityPub specification.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures::StreamExt;
use lapin::{
    Channel, Connection, ConnectionProperties, ExchangeKind, options::*, types::FieldTable,
};
use oxifed::Activity;
use oxifed::client::{ActivityPubClient, ClientConfig};
use oxifed::database::DatabaseManager;
use oxifed::httpsignature::{
    ComponentIdentifier, SignatureAlgorithm, SignatureConfig, SignatureParameters,
};
use oxifed::messaging::EXCHANGE_ACTIVITYPUB_PUBLISH;

use std::sync::Arc;
use thiserror::Error;
use tokio::signal;
use tracing::{error, info, warn};
use url::Url;

/// Publisher daemon errors
#[derive(Error, Debug)]
pub enum PublisherError {
    #[error("AMQP connection error: {0}")]
    AmqpError(#[from] lapin::Error),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),

    #[error("HTTP client error: {0}")]
    ClientError(#[from] oxifed::client::ClientError),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Environment variable error: {0}")]
    EnvError(#[from] std::env::VarError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Publisher daemon configuration
#[derive(Debug, Clone)]
pub struct PublisherConfig {
    pub amqp_url: String,
    pub mongodb_uri: Option<String>,
    pub mongodb_dbname: String,
    pub worker_count: usize,
    pub retry_attempts: usize,
    pub retry_delay_ms: u64,
}

impl Default for PublisherConfig {
    fn default() -> Self {
        Self {
            amqp_url: "amqp://guest:guest@localhost:5672".to_string(),
            mongodb_uri: None,
            mongodb_dbname: "domainservd".to_string(),
            worker_count: 4,
            retry_attempts: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// ActivityPub Publisher Daemon
pub struct PublisherDaemon {
    config: PublisherConfig,
    connection: Connection,
    db_manager: Option<Arc<DatabaseManager>>,
}

impl PublisherDaemon {
    /// Create a new publisher daemon
    pub async fn new(config: PublisherConfig) -> Result<Self, PublisherError> {
        info!("Connecting to AMQP server: {}", config.amqp_url);

        let connection =
            Connection::connect(&config.amqp_url, ConnectionProperties::default()).await?;

        // Initialize MongoDB for key lookups
        let db_manager = if let Some(ref uri) = config.mongodb_uri {
            info!("Connecting to MongoDB for key lookups");
            let client_options = mongodb::options::ClientOptions::parse(uri)
                .await
                .map_err(|e| PublisherError::DatabaseError(e.to_string()))?;
            let mongo_client = mongodb::Client::with_options(client_options)
                .map_err(|e| PublisherError::DatabaseError(e.to_string()))?;
            let database = mongo_client.database(&config.mongodb_dbname);
            let manager = DatabaseManager::new(database);
            Some(Arc::new(manager))
        } else {
            warn!("MONGODB_URI not set - outgoing activities will be unsigned");
            None
        };

        Ok(Self {
            config,
            connection,
            db_manager,
        })
    }

    /// Start the publisher daemon
    pub async fn start(&self) -> Result<(), PublisherError> {
        info!(
            "Starting ActivityPub Publisher Daemon with {} workers",
            self.config.worker_count
        );

        // Create channels for workers
        let mut workers = Vec::new();

        for worker_id in 0..self.config.worker_count {
            let channel = self.connection.create_channel().await?;
            let config = self.config.clone();
            let db_manager = self.db_manager.clone();

            let worker = tokio::spawn(async move {
                if let Err(e) = Self::run_worker(worker_id, channel, db_manager, config).await {
                    error!("Worker {} failed: {}", worker_id, e);
                }
            });

            workers.push(worker);
        }

        info!("All workers started, waiting for shutdown signal");

        // Wait for shutdown signal
        signal::ctrl_c().await?;
        info!("Shutdown signal received, stopping workers");

        // Cancel all workers
        for worker in workers {
            worker.abort();
        }

        info!("Publisher daemon stopped");
        Ok(())
    }

    /// Run a single worker
    async fn run_worker(
        worker_id: usize,
        channel: Channel,
        db_manager: Option<Arc<DatabaseManager>>,
        config: PublisherConfig,
    ) -> Result<(), PublisherError> {
        info!("Starting worker {}", worker_id);

        // Declare the exchange
        channel
            .exchange_declare(
                EXCHANGE_ACTIVITYPUB_PUBLISH,
                ExchangeKind::Fanout,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        // Declare a worker-specific queue
        let queue_name = format!("publisherd.worker.{}", worker_id);
        let queue = channel
            .queue_declare(
                &queue_name,
                QueueDeclareOptions {
                    durable: true,
                    auto_delete: false,
                    exclusive: false,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        // Bind queue to exchange
        channel
            .queue_bind(
                queue.name().as_str(),
                EXCHANGE_ACTIVITYPUB_PUBLISH,
                "",
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;

        // Create consumer
        let consumer = channel
            .basic_consume(
                queue.name().as_str(),
                &format!("publisherd_worker_{}", worker_id),
                BasicConsumeOptions {
                    no_ack: false,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        info!("Worker {} is ready to process activities", worker_id);

        // Process messages using async stream
        consumer
            .for_each(move |delivery_result| {
                let db_manager = db_manager.clone();
                let config = config.clone();

                async move {
                    match delivery_result {
                        Ok(delivery) => {
                            let delivery_tag = delivery.delivery_tag;
                            info!(
                                "Worker {} processing message with delivery tag: {}",
                                worker_id, delivery_tag
                            );

                            match Self::process_activity(&delivery.data, db_manager, config).await {
                                Ok(_) => {
                                    info!(
                                        "Worker {} successfully processed message {}",
                                        worker_id, delivery_tag
                                    );
                                    if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                        error!(
                                            "Worker {} failed to ack message {}: {}",
                                            worker_id, delivery_tag, e
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        "Worker {} failed to process message {}: {}",
                                        worker_id, delivery_tag, e
                                    );
                                    // For certain errors, we might want to requeue, for others not
                                    let should_requeue = match &e {
                                        PublisherError::JsonError(_) => false, // Don't requeue malformed JSON
                                        PublisherError::UrlError(_) => false, // Don't requeue bad URLs
                                        _ => true, // Requeue for network/temporary errors
                                    };

                                    if let Err(e) = delivery
                                        .nack(BasicNackOptions {
                                            requeue: should_requeue,
                                            ..Default::default()
                                        })
                                        .await
                                    {
                                        error!(
                                            "Worker {} failed to nack message {}: {}",
                                            worker_id, delivery_tag, e
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Worker {} failed to receive message: {}", worker_id, e);
                        }
                    }
                }
            })
            .await;

        Ok(())
    }

    /// Build a signing-capable ActivityPubClient for a given actor
    async fn build_signing_client(
        actor_id: &str,
        db_manager: &Option<Arc<DatabaseManager>>,
    ) -> Result<ActivityPubClient, PublisherError> {
        if let Some(db) = db_manager {
            // Look up the actor's key from MongoDB
            match db.find_keys_by_actor(actor_id).await {
                Ok(keys) if !keys.is_empty() => {
                    let key_doc = &keys[0];
                    if let Some(ref private_pem) = key_doc.private_key_pem {
                        // Decode PEM to DER for ring
                        let private_der = {
                            let lines: Vec<&str> = private_pem
                                .lines()
                                .filter(|line| !line.starts_with("-----"))
                                .collect();
                            BASE64.decode(lines.join("")).map_err(|e| {
                                PublisherError::DatabaseError(format!("Invalid PEM base64: {}", e))
                            })?
                        };

                        let key_id = format!("{}#main-key", actor_id);
                        let sig_config = SignatureConfig {
                            algorithm: SignatureAlgorithm::RsaSha256,
                            parameters: SignatureParameters::new(),
                            key_id,
                            components: vec![
                                ComponentIdentifier::RequestTarget,
                                ComponentIdentifier::Header("host".to_string()),
                                ComponentIdentifier::Header("date".to_string()),
                                ComponentIdentifier::Header("content-type".to_string()),
                                ComponentIdentifier::Digest,
                            ],
                            private_key: private_der,
                        };

                        let client_config = ClientConfig {
                            user_agent: "Oxifed/0.3.8".to_string(),
                            http_signature_config: Some(sig_config),
                            oauth_token: None,
                        };

                        info!("Created signing client for actor: {}", actor_id);
                        return ActivityPubClient::with_config(client_config)
                            .map_err(PublisherError::ClientError);
                    }
                    warn!("No private key found for actor: {}", actor_id);
                }
                Ok(_) => {
                    warn!("No key document found for actor: {}", actor_id);
                }
                Err(e) => {
                    warn!("Failed to look up key for actor {}: {}", actor_id, e);
                }
            }
        }

        // Fallback to unsigned client
        warn!("Using unsigned client for delivery");
        ActivityPubClient::new().map_err(PublisherError::ClientError)
    }

    /// Process a single activity
    async fn process_activity(
        data: &[u8],
        db_manager: Option<Arc<DatabaseManager>>,
        config: PublisherConfig,
    ) -> Result<(), PublisherError> {
        // Parse the activity from JSON
        let activity: Activity = serde_json::from_slice(data)?;

        info!(
            "Processing activity: {:?} with ID: {:?}",
            activity.activity_type, activity.id
        );

        // Extract actor ID for signing
        let actor_id = activity.actor.as_ref().map(|a| match a {
            oxifed::ObjectOrLink::Url(url) => url.to_string(),
            oxifed::ObjectOrLink::Object(obj) => {
                obj.id.as_ref().map(|u| u.to_string()).unwrap_or_default()
            }
            oxifed::ObjectOrLink::Link(link) => link
                .href
                .as_ref()
                .map(|u| u.to_string())
                .unwrap_or_default(),
        });

        // Build a signing client for this actor
        let client = if let Some(ref aid) = actor_id {
            Self::build_signing_client(aid, &db_manager).await?
        } else {
            warn!("Activity has no actor - using unsigned client");
            ActivityPubClient::new().map_err(PublisherError::ClientError)?
        };

        // Extract recipients from the activity
        let recipients = Self::extract_recipients(&activity)?;

        if recipients.is_empty() {
            warn!("No recipients found for activity");
            return Ok(());
        }

        info!("Delivering activity to {} recipients", recipients.len());

        // Deliver to each recipient with retry logic
        let mut successful_deliveries = 0;
        let mut failed_deliveries = 0;

        for recipient_url in recipients {
            // Extract inbox URL from recipient
            match Self::get_inbox_url(&recipient_url, &client).await {
                Ok(inbox_url) => {
                    match Self::deliver_with_retry(&client, &inbox_url, &activity, &config).await {
                        Ok(_) => {
                            successful_deliveries += 1;
                        }
                        Err(e) => {
                            error!("Failed to deliver to {}: {}", inbox_url, e);
                            failed_deliveries += 1;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get inbox for {}: {}", recipient_url, e);
                    failed_deliveries += 1;
                }
            }
        }

        info!(
            "Delivery completed. Success: {}, Failed: {}",
            successful_deliveries, failed_deliveries
        );

        Ok(())
    }

    /// Get inbox URL for a given actor URL
    async fn get_inbox_url(
        actor_url: &Url,
        client: &ActivityPubClient,
    ) -> Result<Url, PublisherError> {
        // Fetch the actor to get their inbox
        let actor = client.fetch_actor(actor_url).await?;

        let inbox_str = actor
            .additional_properties
            .get("inbox")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                PublisherError::JsonError(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Actor missing inbox property",
                )))
            })?;

        Ok(Url::parse(inbox_str)?)
    }

    /// Extract recipient URLs from activity addressing
    fn extract_recipients(activity: &Activity) -> Result<Vec<Url>, PublisherError> {
        let mut recipients = Vec::new();

        // Check to field
        if let Some(to_value) = activity.additional_properties.get("to") {
            Self::extract_urls_from_value(to_value, &mut recipients)?;
        }

        // Check cc field
        if let Some(cc_value) = activity.additional_properties.get("cc") {
            Self::extract_urls_from_value(cc_value, &mut recipients)?;
        }

        // For bcc and bto, we process but don't include in final delivery
        // as they should be handled privately by the sender

        // Check audience field
        if let Some(audience_value) = activity.additional_properties.get("audience") {
            Self::extract_urls_from_value(audience_value, &mut recipients)?;
        }

        // Filter out special collections like "https://www.w3.org/ns/activitystreams#Public"
        recipients.retain(|url| {
            !url.as_str()
                .starts_with("https://www.w3.org/ns/activitystreams")
        });

        // Remove duplicates
        recipients.sort();
        recipients.dedup();

        Ok(recipients)
    }

    /// Extract URLs from a JSON value (handles both single strings and arrays)
    fn extract_urls_from_value(
        value: &serde_json::Value,
        recipients: &mut Vec<Url>,
    ) -> Result<(), PublisherError> {
        match value {
            serde_json::Value::String(url_str) => {
                if let Ok(url) = Url::parse(url_str) {
                    // Only include HTTP/HTTPS URLs for actual delivery
                    if url.scheme() == "http" || url.scheme() == "https" {
                        recipients.push(url);
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    if let serde_json::Value::String(url_str) = item
                        && let Ok(url) = Url::parse(url_str)
                    {
                        // Only include HTTP/HTTPS URLs for actual delivery
                        let scheme = url.scheme();
                        if scheme == "http" || scheme == "https" {
                            recipients.push(url.clone());
                        }
                    }
                }
            }
            _ => {
                warn!("Unexpected value type in recipient field: {:?}", value);
            }
        }
        Ok(())
    }

    /// Deliver activity to a single recipient with retry logic
    async fn deliver_with_retry(
        client: &oxifed::client::ActivityPubClient,
        recipient_url: &Url,
        activity: &Activity,
        config: &PublisherConfig,
    ) -> Result<(), PublisherError> {
        let mut attempts = 0;
        let mut last_error = None;

        while attempts < config.retry_attempts {
            attempts += 1;

            match client.send_to_inbox(recipient_url, activity).await {
                Ok(_) => {
                    if attempts > 1 {
                        info!(
                            "Successfully delivered to {} after {} attempts",
                            recipient_url, attempts
                        );
                    }
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempts < config.retry_attempts {
                        let delay = std::time::Duration::from_millis(
                            config.retry_delay_ms * (2_u64.pow(attempts as u32 - 1)),
                        );

                        warn!(
                            "Delivery attempt {} failed for {}, retrying in {:?}",
                            attempts, recipient_url, delay
                        );

                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(PublisherError::ClientError(last_error.unwrap()))
    }
}

/// Load configuration from environment variables
fn load_config() -> PublisherConfig {
    PublisherConfig {
        amqp_url: std::env::var("AMQP_URI")
            .or_else(|_| std::env::var("AMQP_URL"))
            .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string()),
        mongodb_uri: std::env::var("MONGODB_URI").ok(),
        mongodb_dbname: std::env::var("MONGODB_DBNAME")
            .unwrap_or_else(|_| "domainservd".to_string()),
        worker_count: std::env::var("PUBLISHER_WORKERS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(4),
        retry_attempts: std::env::var("PUBLISHER_RETRY_ATTEMPTS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3),
        retry_delay_ms: std::env::var("PUBLISHER_RETRY_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000),
    }
}

#[tokio::main]
async fn main() -> Result<(), PublisherError> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting ActivityPub Publisher Daemon");

    // Load configuration
    let config = load_config();
    info!("Configuration: {:?}", config);

    // Create and start daemon
    let daemon = PublisherDaemon::new(config).await?;
    daemon.start().await?;

    Ok(())
}
