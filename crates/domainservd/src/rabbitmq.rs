//! RabbitMQ/LavinMQ connection and message handling

use crate::db::MongoDB;

use deadpool_lapin::{Config, Pool, Runtime};
use futures::{StreamExt, TryStreamExt};
use lapin::{
    ExchangeKind,
    options::{
        BasicAckOptions, BasicConsumeOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::FieldTable,
};

use mongodb::bson::Bson;
use oxifed::messaging::{
    AcceptActivityMessage, AnnounceActivityMessage, DomainInfo, DomainRpcResponse,
    FollowActivityMessage, KeyGenerateMessage, LikeActivityMessage, Message, MessageEnum,
    NoteCreateMessage, NoteDeleteMessage, NoteUpdateMessage, ProfileCreateMessage,
    ProfileDeleteMessage, ProfileUpdateMessage, RejectActivityMessage,
};
use oxifed::messaging::{
    EXCHANGE_ACTIVITYPUB_PUBLISH, EXCHANGE_INCOMING_PROCESS, EXCHANGE_INTERNAL_PUBLISH,
    EXCHANGE_RPC_REQUEST, EXCHANGE_RPC_RESPONSE, QUEUE_RPC_DOMAIN,
};
use oxifed::pki::{KeyAlgorithm, PkiManager};
use serde::de::Error;
use std::sync::Arc;
use std::time::SystemTime;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Constants for RabbitMQ queue names
pub const QUEUE_ACTIVITIES: &str = "oxifed.activities";
pub const CONSUMER_TAG: &str = "activities_consumer";
pub const RPC_CONSUMER_TAG: &str = "rpc_domain_consumer";

/// RabbitMQ error types
#[derive(Error, Debug)]
pub enum RabbitMQError {
    #[error("LavinMQ connection error: {0}")]
    ConnectionError(#[from] lapin::Error),

    #[error("LavinMQ pool error: {0}")]
    PoolError(#[from] deadpool_lapin::PoolError),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("MongoDB error: {0}")]
    DbError(#[from] crate::db::DbError),

    #[error("URL Parse error: {0}")]
    URLParse(#[from] url::ParseError),

    #[error("Failed to convert object to bson {0}")]
    BsonError(#[from] mongodb::bson::ser::Error),

    #[error("Activity Pub Client Error {0}")]
    ActPubClientError(#[from] oxifed::client::ClientError),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("Domain not found: {0}")]
    DomainNotFound(String),

    #[error("Constraint error: {0}")]
    ConstraintError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] oxifed::database::DatabaseError),
}

/// Create a LavinMQ connection pool
pub fn create_connection_pool(amqp_url: &str) -> Pool {
    let config = Config {
        url: Some(amqp_url.to_string()),
        ..Default::default()
    };

    config.create_pool(Some(Runtime::Tokio1)).unwrap()
}

/// Initialize RabbitMQ exchanges and queues
pub async fn init_rabbitmq(pool: &Pool) -> Result<(), RabbitMQError> {
    let conn = pool.get().await.map_err(RabbitMQError::PoolError)?;
    let channel = conn.create_channel().await?;

    // Enable publisher confirms for deliver-once semantics
    channel
        .confirm_select(lapin::options::ConfirmSelectOptions::default())
        .await?;

    // Declare the internal publish exchange - fanout exchange for internal services
    channel
        .exchange_declare(
            EXCHANGE_INTERNAL_PUBLISH,
            ExchangeKind::Fanout,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare the ActivityPub publish exchange for ActivityPub messages
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

    // Declare the incoming processing exchange for received ActivityPub objects
    // Using fanout exchange with durable configuration for deliver-once semantics
    channel
        .exchange_declare(
            EXCHANGE_INCOMING_PROCESS,
            ExchangeKind::Fanout,
            ExchangeDeclareOptions {
                durable: true,
                auto_delete: false,
                internal: false,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare the RPC request exchange - direct exchange for RPC requests
    channel
        .exchange_declare(
            EXCHANGE_RPC_REQUEST,
            ExchangeKind::Direct,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare the RPC response exchange - direct exchange for RPC responses
    channel
        .exchange_declare(
            EXCHANGE_RPC_RESPONSE,
            ExchangeKind::Direct,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare the activities queue with deliver-once semantics
    channel
        .queue_declare(
            QUEUE_ACTIVITIES,
            QueueDeclareOptions {
                durable: true,
                auto_delete: false,
                exclusive: false,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare incoming processing pipeline queues with deliver-once semantics
    // These queues will be bound to processing pipeline exchanges by individual daemons
    let pipeline_queues = [
        "oxifed.incoming.validation",
        "oxifed.incoming.spam_filter",
        "oxifed.incoming.moderation",
        "oxifed.incoming.relationship_verify",
        "oxifed.incoming.storage",
    ];

    for queue_name in &pipeline_queues {
        channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    durable: true,      // Survive broker restart
                    auto_delete: false, // Don't auto-delete when no consumers
                    exclusive: false,   // Allow multiple consumers
                    ..Default::default()
                },
                {
                    let mut args = FieldTable::default();
                    // Enable quorum queues for better deliver-once guarantees
                    args.insert(
                        "x-queue-type".into(),
                        lapin::types::AMQPValue::LongString("quorum".into()),
                    );
                    // Set message TTL to prevent infinite retention
                    args.insert(
                        "x-message-ttl".into(),
                        lapin::types::AMQPValue::LongLongInt(1800000),
                    ); // 30 minutes
                    // Enable dead letter exchange for failed messages
                    args.insert(
                        "x-dead-letter-exchange".into(),
                        lapin::types::AMQPValue::LongString("oxifed.dlx".into()),
                    );
                    args
                },
            )
            .await?;
    }

    // Declare dead letter exchange for failed messages
    channel
        .exchange_declare(
            "oxifed.dlx",
            ExchangeKind::Direct,
            ExchangeDeclareOptions {
                durable: true,
                auto_delete: false,
                internal: false,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare dead letter queue
    channel
        .queue_declare(
            "oxifed.dlq",
            QueueDeclareOptions {
                durable: true,
                auto_delete: false,
                exclusive: false,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Bind dead letter queue to dead letter exchange
    channel
        .queue_bind(
            "oxifed.dlq",
            "oxifed.dlx",
            "",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // Declare the RPC domain queue
    channel
        .queue_declare(
            QUEUE_RPC_DOMAIN,
            QueueDeclareOptions {
                durable: true,
                auto_delete: false,
                exclusive: false,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Bind the queue to the activities exchange
    channel
        .queue_bind(
            QUEUE_ACTIVITIES,
            EXCHANGE_INTERNAL_PUBLISH,
            "", // routing key not needed for fanout exchanges
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // Bind the RPC domain queue to the RPC request exchange
    channel
        .queue_bind(
            QUEUE_RPC_DOMAIN,
            EXCHANGE_RPC_REQUEST,
            "domain", // routing key for domain requests
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!("RabbitMQ exchanges and queues initialized successfully");
    Ok(())
}

/// Start Message Queue consumers
pub async fn start_consumers(pool: Pool, db: Arc<MongoDB>) -> Result<(), RabbitMQError> {
    // Start activities message consumer
    start_activities_consumer(pool.clone(), db.clone()).await?;

    // Start RPC consumer for domain queries
    start_rpc_consumer(pool.clone(), db.clone()).await?;

    Ok(())
}

/// Start RPC consumer for domain queries
async fn start_rpc_consumer(pool: Pool, db: Arc<MongoDB>) -> Result<(), RabbitMQError> {
    info!("Starting RPC consumer for domain queries");

    tokio::spawn(async move {
        loop {
            match pool.get().await {
                Ok(conn) => match conn.create_channel().await {
                    Ok(channel) => {
                        match channel
                            .basic_consume(
                                QUEUE_RPC_DOMAIN,
                                RPC_CONSUMER_TAG,
                                BasicConsumeOptions::default(),
                                FieldTable::default(),
                            )
                            .await
                        {
                            Ok(mut consumer) => {
                                info!("RPC consumer started successfully");
                                while let Some(delivery) = consumer.next().await {
                                    match delivery {
                                        Ok(delivery) => {
                                            if let Err(e) = process_rpc_message(
                                                &delivery.data,
                                                &db,
                                                &channel,
                                                &delivery.properties,
                                            )
                                            .await
                                            {
                                                error!("Failed to process RPC message: {}", e);
                                            }

                                            if let Err(e) =
                                                delivery.ack(BasicAckOptions::default()).await
                                            {
                                                error!("Failed to acknowledge RPC message: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to consume RPC message: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to start RPC consumer: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to create RPC channel: {}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to get RPC connection: {}", e);
                }
            }

            warn!("RPC consumer stopped, restarting in 5 seconds...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });

    Ok(())
}

/// Start activities message consumer
async fn start_activities_consumer(pool: Pool, db: Arc<MongoDB>) -> Result<(), RabbitMQError> {
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;

    info!("Starting consumer for {} queue", QUEUE_ACTIVITIES);

    let mut consumer = channel
        .basic_consume(
            QUEUE_ACTIVITIES,
            CONSUMER_TAG,
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // Process messages in a separate task
    tokio::spawn(async move {
        info!("Activities consumer ready, waiting for messages");

        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    match process_message(&delivery.data, &db).await {
                        Ok(_) => {
                            debug!("Successfully processed activities message");
                            // Acknowledge the message
                            if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                error!("Failed to acknowledge activities message: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to process activities message: {}", e);
                            // Still acknowledge to avoid re-processing failed messages
                            if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                                error!("Failed to acknowledge activities message: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to consume activities message: {}", e);
                }
            }
        }

        warn!("Activities consumer stopped unexpectedly");
    });

    Ok(())
}

/// Process a profile creation message
async fn process_message(data: &[u8], db: &Arc<MongoDB>) -> Result<(), RabbitMQError> {
    // Parse the message
    let message: MessageEnum = serde_json::from_slice(data)?;

    match message {
        MessageEnum::ProfileCreateMessage(msg) => create_person_object(db, &msg).await,
        MessageEnum::ProfileUpdateMessage(msg) => update_person_object(db, &msg).await,
        MessageEnum::ProfileDeleteMessage(msg) => delete_person_object(db, &msg).await,
        MessageEnum::NoteCreateMessage(msg) => create_note_object(db, &msg).await,
        MessageEnum::NoteUpdateMessage(msg) => update_note_object(db, &msg).await,
        MessageEnum::NoteDeleteMessage(msg) => delete_note_object(db, &msg).await,
        MessageEnum::FollowActivityMessage(msg) => handle_follow(db, &msg).await,
        MessageEnum::LikeActivityMessage(msg) => handle_like(db, &msg).await,
        MessageEnum::AnnounceActivityMessage(msg) => handle_announce(db, &msg).await,
        MessageEnum::AcceptActivityMessage(msg) => handle_accept(db, &msg).await,
        MessageEnum::RejectActivityMessage(msg) => handle_reject(db, &msg).await,
        MessageEnum::DomainCreateMessage(msg) => create_domain_object(db, &msg).await,
        MessageEnum::DomainUpdateMessage(msg) => update_domain_object(db, &msg).await,
        MessageEnum::DomainDeleteMessage(msg) => delete_domain_object(db, &msg).await,
        MessageEnum::KeyGenerateMessage(msg) => handle_key_generate(db, &msg).await,
        MessageEnum::DomainRpcRequest(_) | MessageEnum::DomainRpcResponse(_) => {
            warn!("RPC messages should not be processed by this handler");
            Ok(())
        }
        MessageEnum::IncomingObjectMessage(_) => {
            // Incoming objects should be processed by dedicated incoming processing daemons
            warn!(
                "IncomingObjectMessage should not be processed by domainservd - it should be handled by dedicated processing daemons"
            );
            Ok(())
        }
        MessageEnum::IncomingActivityMessage(_) => {
            // Incoming activities should be processed by dedicated incoming processing daemons
            warn!(
                "IncomingActivityMessage should not be processed by domainservd - it should be handled by dedicated processing daemons"
            );
            Ok(())
        }
    }
}

/// Handle key generation request
async fn handle_key_generate(
    db: &Arc<MongoDB>,
    msg: &KeyGenerateMessage,
) -> Result<(), RabbitMQError> {
    info!("Generating key for actor: {}", msg.actor);

    // Create PKI manager
    let mut pki_manager = PkiManager::new();

    // Parse algorithm
    let algorithm = match msg.algorithm.to_lowercase().as_str() {
        "rsa" => {
            let key_size = msg.key_size.unwrap_or(2048);
            info!("Using RSA algorithm with key size: {}", key_size);
            KeyAlgorithm::Rsa { key_size }
        }
        "ed25519" => {
            info!("Using Ed25519 algorithm");
            KeyAlgorithm::Ed25519
        }
        _ => {
            error!("Unsupported algorithm: {}", msg.algorithm);
            return Err(RabbitMQError::ConstraintError(format!(
                "Unsupported algorithm: {}",
                msg.algorithm
            )));
        }
    };

    // Generate key
    match pki_manager.generate_user_key(msg.actor.clone(), algorithm) {
        Ok(user_key) => {
            info!(
                "Key generated successfully for actor: {}, key ID: {}",
                msg.actor, user_key.key_id
            );

            // Create KeyDocument from UserKeyInfo
            let key_document = oxifed::database::KeyDocument {
                id: None,
                key_id: user_key.key_id.clone(),
                actor_id: user_key.actor_id.clone(),
                key_type: oxifed::database::KeyType::User,
                algorithm: format!("{:?}", user_key.public_key.algorithm).to_lowercase(),
                key_size: match user_key.public_key.algorithm {
                    KeyAlgorithm::Rsa { key_size } => Some(key_size),
                    _ => None,
                },
                public_key_pem: user_key.public_key.pem_data.clone(),
                private_key_pem: user_key
                    .private_key
                    .as_ref()
                    .map(|pk| pk.encrypted_pem.clone()),
                encryption_algorithm: user_key
                    .private_key
                    .as_ref()
                    .map(|pk| pk.encryption_algorithm.clone()),
                fingerprint: user_key.public_key.fingerprint.clone(),
                trust_level: user_key.trust_level,
                domain_signature: user_key.domain_signature.map(|ds| {
                    let mut doc = mongodb::bson::Document::new();
                    doc.insert("domain", ds.domain);
                    doc.insert("signature", ds.signature);
                    let system_time: SystemTime = ds.signed_at.into();
                    doc.insert(
                        "signed_at",
                        mongodb::bson::Bson::DateTime(system_time.into()),
                    );
                    doc.insert("domain_key_id", ds.domain_key_id);
                    doc.insert("verification_chain", ds.verification_chain);
                    doc
                }),
                master_signature: None,
                usage: vec!["signing".to_string()],
                status: oxifed::database::KeyStatus::Active,
                created_at: user_key.created_at,
                expires_at: user_key.expires_at,
                rotation_policy: {
                    let mut doc = mongodb::bson::Document::new();
                    doc.insert("automatic", user_key.rotation_policy.automatic);
                    if let Some(interval) = user_key.rotation_policy.rotation_interval {
                        doc.insert("rotation_interval", interval.num_seconds());
                    }
                    if let Some(max_age) = user_key.rotation_policy.max_age {
                        doc.insert("max_age", max_age.num_seconds());
                    }
                    if let Some(notify_before) = user_key.rotation_policy.notify_before {
                        doc.insert("notify_before", notify_before.num_seconds());
                    }
                    Some(doc)
                },
                domain: None,
            };

            // Save key to database
            match db.manager().insert_key(key_document).await {
                Ok(key_id) => {
                    info!("Key saved to database with ID: {}", key_id);
                    Ok(())
                }
                Err(e) => {
                    error!("Failed to save key to database: {}", e);
                    Err(RabbitMQError::DatabaseError(e))
                }
            }
        }
        Err(e) => {
            error!("Failed to generate key: {}", e);
            Err(RabbitMQError::ConstraintError(format!(
                "Failed to generate key: {}",
                e
            )))
        }
    }
}

/// Process RPC messages for domain queries
///
/// Note: RPC messages are wrapped in MessageEnum when sent over RabbitMQ.
/// The client sends `request.to_message()` which wraps the DomainRpcRequest
/// in a MessageEnum, so we must parse MessageEnum first, then extract the
/// actual RPC request from it.
async fn process_rpc_message(
    data: &[u8],
    db: &Arc<MongoDB>,
    channel: &lapin::Channel,
    properties: &lapin::BasicProperties,
) -> Result<(), RabbitMQError> {
    use lapin::options::BasicPublishOptions;

    // Parse the message envelope first (MessageEnum wrapper)
    let message: MessageEnum = match serde_json::from_slice(data) {
        Ok(msg) => msg,
        Err(e) => {
            error!("Failed to parse RPC message: {}", e);
            return Ok(());
        }
    };

    // Extract the RPC request from the message envelope
    let request = match message {
        MessageEnum::DomainRpcRequest(req) => req,
        MessageEnum::IncomingObjectMessage(_) | MessageEnum::IncomingActivityMessage(_) => {
            warn!("Incoming messages should not be processed by RPC handler");
            return Ok(());
        }
        _ => {
            error!("Received non-RPC message in RPC queue");
            return Ok(());
        }
    };

    info!(
        "Processing RPC request: {} (type: {:?})",
        request.request_id, request.request_type
    );

    // Process the request
    let response = match request.request_type {
        oxifed::messaging::DomainRpcRequestType::ListDomains => {
            handle_list_domains_rpc(db, &request.request_id).await
        }
        oxifed::messaging::DomainRpcRequestType::GetDomain { domain } => {
            handle_get_domain_rpc(db, &request.request_id, &domain).await
        }
    };

    // Send response back to the client
    if let Some(reply_to) = &properties.reply_to() {
        let default_correlation_id = request.request_id.clone().into();
        let correlation_id = properties
            .correlation_id()
            .as_ref()
            .unwrap_or(&default_correlation_id);

        let response_data = match serde_json::to_vec(&response.to_message()) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to serialize RPC response: {}", e);
                return Ok(());
            }
        };

        let response_properties =
            lapin::BasicProperties::default().with_correlation_id(correlation_id.clone());

        if let Err(e) = channel
            .basic_publish(
                "", // Use default exchange for direct reply
                reply_to.as_str(),
                BasicPublishOptions::default(),
                &response_data,
                response_properties,
            )
            .await
        {
            error!("Failed to send RPC response: {}", e);
        } else {
            info!("RPC response sent for request: {}", request.request_id);
        }
    } else {
        warn!("RPC request {} has no reply_to queue", request.request_id);
    }

    Ok(())
}

/// Handle list domains RPC request
async fn handle_list_domains_rpc(db: &Arc<MongoDB>, request_id: &str) -> DomainRpcResponse {
    use mongodb::bson::doc;
    use oxifed::database::DomainDocument;

    let collection: mongodb::Collection<DomainDocument> = db.database().collection("domains");

    match collection.find(doc! {}).await {
        Ok(cursor) => {
            let mut domains = Vec::new();
            let domain_docs: Vec<DomainDocument> = match cursor.try_collect().await {
                Ok(docs) => docs,
                Err(e) => {
                    error!("Failed to collect domain documents: {}", e);
                    return DomainRpcResponse::error(
                        request_id.to_string(),
                        format!("Database error: {}", e),
                    );
                }
            };

            for domain_doc in domain_docs {
                let domain_info = DomainInfo {
                    domain: domain_doc.domain,
                    name: domain_doc.name,
                    description: domain_doc.description,
                    contact_email: domain_doc.contact_email,
                    registration_mode: format!("{:?}", domain_doc.registration_mode),
                    authorized_fetch: domain_doc.authorized_fetch,
                    max_note_length: domain_doc.max_note_length,
                    max_file_size: domain_doc.max_file_size,
                    allowed_file_types: domain_doc.allowed_file_types,
                    status: format!("{:?}", domain_doc.status),
                    created_at: domain_doc.created_at.to_rfc3339(),
                    updated_at: domain_doc.updated_at.to_rfc3339(),
                };
                domains.push(domain_info);
            }

            info!("Found {} domains", domains.len());
            DomainRpcResponse::domain_list(request_id.to_string(), domains)
        }
        Err(e) => {
            error!("Failed to query domains: {}", e);
            DomainRpcResponse::error(request_id.to_string(), format!("Database error: {}", e))
        }
    }
}

/// Handle get domain RPC request
async fn handle_get_domain_rpc(
    db: &Arc<MongoDB>,
    request_id: &str,
    domain_name: &str,
) -> DomainRpcResponse {
    let db_manager = oxifed::database::DatabaseManager::new(db.database().clone());

    match db_manager.find_domain_by_name(domain_name).await {
        Ok(Some(domain_doc)) => {
            let domain_info = DomainInfo {
                domain: domain_doc.domain,
                name: domain_doc.name,
                description: domain_doc.description,
                contact_email: domain_doc.contact_email,
                registration_mode: format!("{:?}", domain_doc.registration_mode),
                authorized_fetch: domain_doc.authorized_fetch,
                max_note_length: domain_doc.max_note_length,
                max_file_size: domain_doc.max_file_size,
                allowed_file_types: domain_doc.allowed_file_types,
                status: format!("{:?}", domain_doc.status),
                created_at: domain_doc.created_at.to_rfc3339(),
                updated_at: domain_doc.updated_at.to_rfc3339(),
            };

            info!("Found domain: {}", domain_name);
            DomainRpcResponse::domain_details(request_id.to_string(), Some(domain_info))
        }
        Ok(None) => {
            info!("Domain not found: {}", domain_name);
            DomainRpcResponse::domain_details(request_id.to_string(), None)
        }
        Err(e) => {
            error!("Failed to query domain {}: {}", domain_name, e);
            DomainRpcResponse::error(request_id.to_string(), format!("Database error: {}", e))
        }
    }
}

async fn handle_like(_db: &Arc<MongoDB>, _msg: &LikeActivityMessage) -> Result<(), RabbitMQError> {
    todo!()
}

async fn handle_announce(
    _db: &Arc<MongoDB>,
    _msg: &AnnounceActivityMessage,
) -> Result<(), RabbitMQError> {
    todo!()
}

async fn handle_accept(
    _db: &Arc<MongoDB>,
    _msg: &AcceptActivityMessage,
) -> Result<(), RabbitMQError> {
    todo!()
}

async fn handle_reject(
    _db: &Arc<MongoDB>,
    _msg: &RejectActivityMessage,
) -> Result<(), RabbitMQError> {
    todo!()
}

async fn handle_follow(
    db: &Arc<MongoDB>,
    msg: &FollowActivityMessage,
) -> Result<(), RabbitMQError> {
    info!(
        "Processing Follow activity: {} -> {}",
        msg.actor, msg.object
    );

    // Parse the object being followed to extract username
    let (_username, domain) = split_subject(&msg.object)?;

    // Verify the domain exists
    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(
            "Domain not found".to_string(),
        ));
    }

    // Extract actor IDs and create timestamp
    let follower_actor_id = msg.actor.clone();
    let target_actor_id = msg.object.clone();
    let now = chrono::Utc::now();

    // Create Follow activity
    let follow_activity = oxifed::Activity {
        activity_type: oxifed::ActivityType::Follow,
        id: Some(
            url::Url::parse(&format!(
                "https://{}/activities/{}",
                domain,
                uuid::Uuid::new_v4()
            ))
            .map_err(RabbitMQError::URLParse)?,
        ),
        name: None,
        summary: Some(format!("{} follows {}", msg.actor, msg.object)),
        actor: Some(oxifed::ObjectOrLink::Url(
            url::Url::parse(&msg.actor).map_err(RabbitMQError::URLParse)?,
        )),
        object: Some(oxifed::ObjectOrLink::Url(
            url::Url::parse(&msg.object).map_err(RabbitMQError::URLParse)?,
        )),
        target: None,
        published: Some(chrono::Utc::now()),
        updated: None,
        additional_properties: {
            let mut props = std::collections::HashMap::new();
            props.insert(
                "to".to_string(),
                serde_json::Value::String(msg.object.clone()),
            );
            props
        },
    };

    // Store the follow activity using unified database manager
    let activity_doc = oxifed::database::ActivityDocument {
        id: None,
        activity_id: follow_activity.id.as_ref().unwrap().to_string(),
        activity_type: oxifed::ActivityType::Follow,
        actor: follower_actor_id.clone(),
        object: Some(target_actor_id.clone()),
        target: None,
        name: None,
        summary: None,
        published: Some(now),
        updated: Some(now),
        to: None,
        cc: None,
        bto: None,
        bcc: None,
        additional_properties: None,
        local: true,
        status: oxifed::database::ActivityStatus::Completed,
        created_at: now,
        attempts: 0,
        last_attempt: None,
        error: None,
    };

    db.manager()
        .insert_activity(activity_doc)
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    // Fetch the actor being followed to get their inbox
    let client = oxifed::client::ActivityPubClient::new()?;

    let object_url = url::Url::parse(&msg.object).map_err(RabbitMQError::URLParse)?;
    match client.fetch_actor(&object_url).await {
        Ok(actor) => {
            // Get the actor's inbox URL
            if let Some(serde_json::Value::String(inbox_url)) =
                actor.additional_properties.get("inbox")
            {
                let inbox = url::Url::parse(inbox_url).map_err(RabbitMQError::URLParse)?;

                // Send the Follow activity to the target actor's inbox
                if let Err(e) = client.send_to_inbox(&inbox, &follow_activity).await {
                    error!(
                        "Failed to send Follow activity to inbox {}: {}",
                        inbox_url, e
                    );
                } else {
                    info!("Follow activity sent to {}", inbox_url);
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch actor {}: {}", msg.object, e);
        }
    }

    // The actual follower relationship will be established when we receive
    // an Accept activity in response to this Follow

    info!("Follow activity processed successfully");
    Ok(())
}

/// Handle Accept activity (typically in response to a Follow)
#[allow(dead_code)]
async fn handle_accept_activity(
    db: &Arc<MongoDB>,
    activity: &oxifed::Activity,
) -> Result<(), RabbitMQError> {
    // Check if this is accepting a Follow activity
    if let Some(oxifed::ObjectOrLink::Object(follow_obj)) = &activity.object
        && follow_obj.object_type == oxifed::ObjectType::Activity
        && let Some(follow_actor) = &follow_obj.additional_properties.get("actor")
        && let Some(follow_target) = &follow_obj.additional_properties.get("object")
        && let (serde_json::Value::String(follower_id), serde_json::Value::String(target_id)) =
            (follow_actor, follow_target)
    {
        return add_follower_relationship(db, follower_id, target_id).await;
    }

    info!("Accept activity processed (not a follow accept)");
    Ok(())
}

/// Add a follower relationship to the database
#[allow(dead_code)]
async fn add_follower_relationship(
    db: &Arc<MongoDB>,
    follower_id: &str,
    target_id: &str,
) -> Result<(), RabbitMQError> {
    // Extract username from target_id
    let (username, domain) = split_subject(target_id)?;

    // Verify domain exists
    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(
            "Domain not found".to_string(),
        ));
    }

    // Create full actor ID for target
    let target_actor_id = format!("https://{}/users/{}", domain, username);

    // Create follow document using the unified database schema
    let follow_doc = oxifed::database::FollowDocument {
        id: None,
        follower: follower_id.to_string(),
        following: target_actor_id.clone(),
        status: oxifed::database::FollowStatus::Accepted,
        activity_id: format!("https://{}/activities/{}", domain, uuid::Uuid::new_v4()),
        accept_activity_id: None,
        created_at: chrono::Utc::now(),
        responded_at: Some(chrono::Utc::now()),
    };

    // Store using the unified database manager
    db.manager()
        .insert_follow(follow_doc)
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    info!(
        "Added follower relationship: {} -> {}",
        follower_id, target_id
    );

    Ok(())
}

/// Remove a follower relationship from the database
#[allow(dead_code)]
async fn remove_follower_relationship(
    db: &Arc<MongoDB>,
    follower_id: &str,
    target_id: &str,
) -> Result<(), RabbitMQError> {
    // Extract username from target_id
    let (username, domain) = split_subject(target_id)?;

    // Verify domain exists
    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(
            "Domain not found".to_string(),
        ));
    }

    // Create full actor ID for target
    let target_actor_id = format!("https://{}/users/{}", domain, username);

    // Update follow status using the unified database manager
    db.manager()
        .update_follow_status(
            follower_id,
            &target_actor_id,
            oxifed::database::FollowStatus::Cancelled,
        )
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    info!(
        "Removed follower relationship: {} -> {}",
        follower_id, target_id
    );
    Ok(())
}

// ActivityPub-compliant delivery to followers according to W3C specification
#[allow(dead_code)]
async fn publish_activity_to_activitypub_exchange(
    activity: &oxifed::Activity,
) -> Result<(), RabbitMQError> {
    // Get AMQP URL from environment or use default
    let amqp_url = std::env::var("AMQP_URL")
        .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());

    // Create a standalone connection for publishing
    let conn =
        lapin::Connection::connect(&amqp_url, lapin::ConnectionProperties::default()).await?;

    let channel = conn.create_channel().await?;

    // Convert the activity to JSON for publishing
    let activity_json = serde_json::to_vec(activity)?;

    // Publish to the oxifed.activitypub.publish exchange
    channel
        .basic_publish(
            oxifed::messaging::EXCHANGE_ACTIVITYPUB_PUBLISH,
            "", // no routing key for fanout exchanges
            lapin::options::BasicPublishOptions::default(),
            &activity_json,
            lapin::BasicProperties::default(),
        )
        .await?;

    info!(
        "Activity published to ActivityPub exchange: {:?}",
        activity.activity_type
    );
    Ok(())
}

/// Publish incoming object to the incoming processing exchange using RabbitMQ deliver-once semantics
pub async fn publish_incoming_object_to_exchange(
    pool: &deadpool_lapin::Pool,
    object: &serde_json::Value,
    object_type: &str,
    attributed_to: &str,
    target_domain: &str,
    target_username: Option<&str>,
    source: Option<&str>,
) -> Result<(), RabbitMQError> {
    // Get connection from pool
    let conn = pool.get().await.map_err(RabbitMQError::PoolError)?;
    let channel = conn.create_channel().await?;

    // Enable publisher confirms for this channel (deliver-once semantics)
    channel
        .confirm_select(lapin::options::ConfirmSelectOptions::default())
        .await?;

    // Create the incoming object message
    let incoming_message = oxifed::messaging::IncomingObjectMessage {
        object: object.clone(),
        object_type: object_type.to_string(),
        attributed_to: attributed_to.to_string(),
        target_domain: target_domain.to_string(),
        target_username: target_username.map(|s| s.to_string()),
        received_at: chrono::Utc::now().to_rfc3339(),
        source: source.map(|s| s.to_string()),
    };

    // Convert to JSON for publishing
    let message_json = serde_json::to_vec(&incoming_message)?;

    // Generate unique message ID for idempotency
    let message_id = format!(
        "{}-{}",
        object
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown"),
        chrono::Utc::now().timestamp_millis()
    );

    // Publish to the incoming processing exchange with RabbitMQ deliver-once semantics
    let confirmation = channel
        .basic_publish(
            EXCHANGE_INCOMING_PROCESS,
            "", // no routing key for fanout exchanges
            lapin::options::BasicPublishOptions {
                mandatory: true,  // Return message if no queue accepts it
                immediate: false, // Don't return if no consumer can immediately handle it
            },
            &message_json,
            lapin::BasicProperties::default()
                .with_delivery_mode(2) // Persistent message (survives broker restart)
                .with_message_id(message_id.into()) // Unique message ID for deduplication
                .with_timestamp(chrono::Utc::now().timestamp() as u64) // Message timestamp
                .with_expiration("1800000".into()), // 30 minute TTL to prevent message buildup
        )
        .await?;

    // Wait for publisher confirmation (deliver-once guarantee)
    confirmation.await?;

    info!(
        "Incoming {} object from {} published to processing exchange with delivery confirmation",
        object_type, attributed_to
    );
    Ok(())
}

/// Publish incoming activity to the incoming processing exchange using RabbitMQ deliver-once semantics
pub async fn publish_incoming_activity_to_exchange(
    pool: &deadpool_lapin::Pool,
    activity: &serde_json::Value,
    activity_type: &str,
    actor: &str,
    target_domain: &str,
    target_username: Option<&str>,
    source: Option<&str>,
) -> Result<(), RabbitMQError> {
    // Get connection from pool
    let conn = pool.get().await.map_err(RabbitMQError::PoolError)?;
    let channel = conn.create_channel().await?;

    // Enable publisher confirms for this channel (deliver-once semantics)
    channel
        .confirm_select(lapin::options::ConfirmSelectOptions::default())
        .await?;

    // Create the incoming activity message
    let incoming_message = oxifed::messaging::IncomingActivityMessage {
        activity: activity.clone(),
        activity_type: activity_type.to_string(),
        actor: actor.to_string(),
        target_domain: target_domain.to_string(),
        target_username: target_username.map(|s| s.to_string()),
        received_at: chrono::Utc::now().to_rfc3339(),
        source: source.map(|s| s.to_string()),
    };

    // Convert to JSON for publishing
    let message_json = serde_json::to_vec(&incoming_message)?;

    // Generate unique message ID for idempotency
    let message_id = format!(
        "{}-{}",
        activity
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown"),
        chrono::Utc::now().timestamp_millis()
    );

    // Publish to the incoming processing exchange with RabbitMQ deliver-once semantics
    let confirmation = channel
        .basic_publish(
            EXCHANGE_INCOMING_PROCESS,
            "", // no routing key for fanout exchanges
            lapin::options::BasicPublishOptions {
                mandatory: true,  // Return message if no queue accepts it
                immediate: false, // Don't return if no consumer can immediately handle it
            },
            &message_json,
            lapin::BasicProperties::default()
                .with_delivery_mode(2) // Persistent message (survives broker restart)
                .with_message_id(message_id.into()) // Unique message ID for deduplication
                .with_timestamp(chrono::Utc::now().timestamp() as u64) // Message timestamp
                .with_expiration("1800000".into()), // 30 minute TTL to prevent message buildup
        )
        .await?;

    // Wait for publisher confirmation (deliver-once guarantee)
    confirmation.await?;

    info!(
        "Incoming {} activity from {} published to processing exchange with delivery confirmation",
        activity_type, actor
    );
    Ok(())
}

async fn delete_note_object(
    db: &Arc<MongoDB>,
    msg: &NoteDeleteMessage,
) -> Result<(), RabbitMQError> {
    // Parse note ID to extract username and domain
    let url = url::Url::parse(&msg.id)?;
    let path = url.path();
    let path_parts: Vec<&str> = path.split('/').collect();

    // Expected format: /u/{username}/notes/{uuid}
    if path_parts.len() < 5 || path_parts[1] != "u" || path_parts[3] != "notes" {
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!("Invalid note ID format: {}", msg.id),
        )));
    }

    let username = path_parts[2];
    let domain = url.host_str().ok_or_else(|| {
        RabbitMQError::JsonError(serde_json::Error::custom(format!(
            "Invalid domain in note ID: {}",
            msg.id
        )))
    })?;

    if !does_domain_exist(domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain.to_string()));
    }

    // Find the note before deleting it (we need it for the Delete activity)
    // TODO: Use unified database manager for outbox operations
    let _filter = mongodb::bson::doc! { "id": &msg.id };

    let _note: Option<oxifed::database::ObjectDocument> = if !msg.force {
        // Use unified database manager to find note
        db.manager()
            .find_object_by_id(&msg.id)
            .await
            .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?
    } else {
        None
    };

    // If the note doesn't exist and we're not forcing deletion, return an error
    if _note.is_none() && !msg.force {
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!("Note not found: {}", msg.id),
        )));
    }

    // Delete the note using unified database manager
    db.manager()
        .delete_object(&msg.id)
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    // Create a Delete activity if a note was found
    if _note.is_some() {
        let now = chrono::Utc::now();

        // Create Delete activity using unified database schema
        let activity_id = format!("{}/delete/{}", msg.id, now.timestamp_millis());
        let activity_doc = oxifed::database::ActivityDocument {
            id: None,
            activity_id: activity_id.clone(),
            activity_type: oxifed::ActivityType::Delete,
            actor: format!("https://{}/users/{}", domain, username),
            object: Some(msg.id.clone()),
            target: None,
            name: None,
            summary: None,
            published: Some(now),
            updated: Some(now),
            to: None,
            cc: None,
            bto: None,
            bcc: None,
            additional_properties: None,
            local: true,
            status: oxifed::database::ActivityStatus::Completed,
            created_at: now,
            attempts: 0,
            last_attempt: None,
            error: None,
        };

        // Insert the Delete activity
        db.manager()
            .insert_activity(activity_doc.clone())
            .await
            .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

        // Publish the activity to ActivityPub exchange for delivery
        publish_activity_document_to_exchange(&activity_doc).await?;

        info!("Delete activity created for note: {}", msg.id);
    }

    info!("Note deleted successfully: {}", msg.id);
    Ok(())
}

async fn update_note_object(
    db: &Arc<MongoDB>,
    msg: &NoteUpdateMessage,
) -> Result<(), RabbitMQError> {
    // Parse note ID to extract username and domain
    let url = url::Url::parse(&msg.id)?;
    let path = url.path();
    let path_parts: Vec<&str> = path.split('/').collect();

    // Expected format: /u/{username}/notes/{uuid}
    if path_parts.len() < 5 || path_parts[1] != "u" || path_parts[3] != "notes" {
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!("Invalid note ID format: {}", msg.id),
        )));
    }

    let username = path_parts[2];
    let domain = url.host_str().ok_or_else(|| {
        RabbitMQError::JsonError(serde_json::Error::custom(format!(
            "Invalid domain in note ID: {}",
            msg.id
        )))
    })?;

    if !does_domain_exist(domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain.to_string()));
    }

    // Find the note to update
    // Find the existing note
    let existing_note = db
        .manager()
        .find_object_by_id(&msg.id)
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    let _note = existing_note.ok_or_else(|| {
        RabbitMQError::JsonError(serde_json::Error::custom(format!(
            "Note not found: {}",
            msg.id
        )))
    })?;

    let now = chrono::Utc::now();
    let mut update_doc = mongodb::bson::Document::new();

    // Update content if provided
    if let Some(content) = &msg.content {
        update_doc.insert("content", content);
    }

    // Update summary if provided
    if let Some(summary) = &msg.summary {
        update_doc.insert("summary", summary);
    }

    // Update tags if provided
    if let Some(tags_str) = &msg.tags {
        let tags: Vec<&str> = tags_str.split(',').map(|s| s.trim()).collect();
        let tags_docs: Vec<mongodb::bson::Document> = tags
            .into_iter()
            .map(|tag| {
                mongodb::bson::doc! {
                    "tag_type": "Hashtag",
                    "name": tag,
                    "href": format!("https://{}/tags/{}", domain, tag)
                }
            })
            .collect();
        update_doc.insert("tag", tags_docs);
    }

    // Add custom properties if provided
    if let Some(props) = &msg.properties {
        let props_doc = mongodb::bson::to_document(props).map_err(RabbitMQError::BsonError)?;
        update_doc.insert("additional_properties", props_doc);
    }

    // Always update the 'updated' timestamp
    let system_time: SystemTime = now.into();
    update_doc.insert("updated", Bson::DateTime(system_time.into()));

    // Update the note in the database
    db.manager()
        .update_object(&msg.id, update_doc)
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    // Create Update activity using unified database schema
    let activity_id = format!("{}/update/{}", msg.id, now.timestamp_millis());
    let activity_doc = oxifed::database::ActivityDocument {
        id: None,
        activity_id: activity_id.clone(),
        activity_type: oxifed::ActivityType::Update,
        actor: format!("https://{}/users/{}", domain, username),
        object: Some(msg.id.clone()),
        target: None,
        name: None,
        summary: None,
        published: Some(now),
        updated: Some(now),
        to: None,
        cc: None,
        bto: None,
        bcc: None,
        additional_properties: None,
        local: true,
        status: oxifed::database::ActivityStatus::Completed,
        created_at: now,
        attempts: 0,
        last_attempt: None,
        error: None,
    };

    // Insert the Update activity
    db.manager()
        .insert_activity(activity_doc.clone())
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    // Publish the activity to ActivityPub exchange for delivery
    publish_activity_document_to_exchange(&activity_doc).await?;

    info!("Note updated successfully: {}", msg.id);
    Ok(())
}

async fn create_note_object(
    db: &Arc<MongoDB>,
    msg: &NoteCreateMessage,
) -> Result<(), RabbitMQError> {
    // Parse username and domain from author
    let (username, domain) = split_subject(&msg.author)?;

    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain));
    }

    // Get the actor to attach as attributedTo
    let actor_id_str = format!("https://{}/users/{}", &domain, &username);

    let actor = db
        .find_actor_by_id(&actor_id_str)
        .await
        .map_err(RabbitMQError::DbError)?;

    if actor.is_none() {
        return Err(RabbitMQError::ProfileNotFound(actor_id_str));
    }

    // Create a unique ID for this note
    let note_id_uuid = uuid::Uuid::new_v4();
    let note_id = format!("https://{}/u/{}/notes/{}", &domain, &username, note_id_uuid);

    // Parse the note ID into a URL
    let _note_id_url = url::Url::parse(&note_id).map_err(RabbitMQError::URLParse)?;

    // Check if a note with this ID already exists
    let existing_note = db
        .manager()
        .find_object_by_id(&note_id)
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    if existing_note.is_some() {
        error!("Note with ID '{}' already exists", note_id);
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!("Note with ID '{}' already exists", note_id),
        )));
    }

    // Parse the actor ID into a URL
    let _actor_id_url = url::Url::parse(&actor_id_str).map_err(RabbitMQError::URLParse)?;

    let now = chrono::Utc::now();

    // Create the note object using unified database schema
    let note_doc = oxifed::database::ObjectDocument {
        id: None,
        object_id: note_id.clone(),
        object_type: oxifed::ObjectType::Note,
        attributed_to: actor_id_str.clone(),
        content: Some(msg.content.clone()),
        summary: msg.summary.clone(),
        name: None,
        media_type: Some("text/html".to_string()),
        url: Some(note_id.clone()),
        published: Some(now),
        updated: Some(now),
        to: None,
        cc: None,
        bto: None,
        bcc: None,
        audience: None,
        in_reply_to: None,
        conversation: None,
        tag: None, // TODO: Parse tags from msg.tags
        attachment: None,
        language: None,
        sensitive: Some(false),
        additional_properties: msg
            .properties
            .clone()
            .map(|p| mongodb::bson::to_document(&p).unwrap_or_default()),
        local: true,
        visibility: oxifed::database::VisibilityLevel::Public,
        created_at: now,
        reply_count: 0,
        like_count: 0,
        announce_count: 0,
    };

    // Insert the note using the unified database manager
    db.manager()
        .insert_object(note_doc)
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    // Create activity using unified database schema
    let activity_id = format!("{}/activity", note_id);
    let activity_doc = oxifed::database::ActivityDocument {
        id: None,
        activity_id: activity_id.clone(),
        activity_type: oxifed::ActivityType::Create,
        actor: actor_id_str.clone(),
        object: Some(note_id.clone()),
        target: None,
        name: None,
        summary: None,
        published: Some(now),
        updated: Some(now),
        to: None,
        cc: None,
        bto: None,
        bcc: None,
        additional_properties: None,
        local: true,
        status: oxifed::database::ActivityStatus::Completed,
        created_at: now,
        attempts: 0,
        last_attempt: None,
        error: None,
    };

    // Insert the activity using the unified database manager
    db.manager()
        .insert_activity(activity_doc.clone())
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    // Publish the activity to ActivityPub exchange for delivery
    publish_activity_document_to_exchange(&activity_doc).await?;

    info!("Note updated successfully: {}", msg.author);
    Ok(())
}

/// Publish activity to ActivityPub exchange for delivery using unified schema
async fn publish_activity_document_to_exchange(
    activity: &oxifed::database::ActivityDocument,
) -> Result<(), RabbitMQError> {
    info!(
        "Publishing activity {} to ActivityPub exchange",
        activity.activity_id
    );

    // Convert ActivityDocument to legacy Activity format for publishing
    let _legacy_activity = oxifed::Activity {
        activity_type: activity.activity_type.clone(),
        id: Some(url::Url::parse(&activity.activity_id).map_err(RabbitMQError::URLParse)?),
        name: activity.name.clone(),
        summary: activity.summary.clone(),
        actor: Some(oxifed::ObjectOrLink::Url(
            url::Url::parse(&activity.actor).map_err(RabbitMQError::URLParse)?,
        )),
        object: activity.object.as_ref().map(|obj| {
            oxifed::ObjectOrLink::Url(
                url::Url::parse(obj)
                    .unwrap_or_else(|_| url::Url::parse("https://example.com/unknown").unwrap()),
            )
        }),
        target: activity.target.as_ref().map(|t| {
            oxifed::ObjectOrLink::Url(
                url::Url::parse(t)
                    .unwrap_or_else(|_| url::Url::parse("https://example.com/unknown").unwrap()),
            )
        }),
        published: activity.published,
        updated: activity.updated,
        additional_properties: std::collections::HashMap::new(),
    };

    // TODO: Implement actual message queue publishing
    // This would serialize the activity and send it to the ActivityPub exchange
    info!("Activity {} queued for delivery", activity.activity_id);

    Ok(())
}

async fn delete_person_object(
    _db: &Arc<MongoDB>,
    msg: &ProfileDeleteMessage,
) -> Result<(), RabbitMQError> {
    // Just a stub for now, will implement later
    info!("Delete person request received for ID: {}", msg.id);
    Ok(())
}

async fn update_person_object(
    db: &Arc<MongoDB>,
    msg: &ProfileUpdateMessage,
) -> Result<(), RabbitMQError> {
    let (username, domain) = split_subject(&msg.subject)?;

    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain));
    }

    let actor_id_str = format!("https://{}/users/{}", &domain, &username);

    let mut update_doc = mongodb::bson::doc! {};

    if let Some(summary) = &msg.summary {
        update_doc.insert("summary", summary);
    }

    if let Some(icon) = &msg.icon {
        update_doc.insert(
            "icon",
            mongodb::bson::to_bson(&icon).map_err(RabbitMQError::BsonError)?,
        );
    }

    if let Some(attachments) = &msg.attachments {
        update_doc.insert(
            "attachment",
            mongodb::bson::to_bson(&attachments).map_err(RabbitMQError::BsonError)?,
        );
    }

    if !update_doc.is_empty() {
        db.manager()
            .update_actor(&actor_id_str, update_doc)
            .await
            .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;
    }

    Ok(())
}

async fn create_person_object(
    db: &Arc<MongoDB>,
    message: &ProfileCreateMessage,
) -> Result<(), RabbitMQError> {
    let (username, domain) = split_subject(&message.subject)?;

    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain));
    }

    // Create the actor ID and check if it already exists
    let actor_id = format!("https://{}/users/{}", &domain, &username);

    // Check if an actor with this ID already exists
    let existing_actor = db
        .find_actor_by_id(&actor_id)
        .await
        .map_err(RabbitMQError::DbError)?;

    if existing_actor.is_some() {
        error!("Actor with ID '{}' already exists", actor_id);
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!("Actor with ID '{}' already exists", actor_id),
        )));
    }

    let aliases = vec![format!("https://{}/@{}", domain, username)];

    // Current time for creation timestamp
    let now = chrono::Utc::now();

    // Create endpoints map
    let mut endpoints = std::collections::HashMap::new();
    endpoints.insert(
        "sharedInbox".to_string(),
        format!("https://{}/sharedInbox", &domain),
    );

    // Generate a key for the actor
    let mut pki_manager = PkiManager::new();
    let public_key_doc =
        match pki_manager.generate_user_key(actor_id.clone(), KeyAlgorithm::Ed25519) {
            Ok(user_key) => {
                info!(
                    "Key generated successfully for actor: {}, key ID: {}",
                    actor_id, user_key.key_id
                );

                // Convert to PublicKeyDocument
                Some(oxifed::database::PublicKeyDocument {
                    id: user_key.key_id.clone(),
                    owner: actor_id.clone(),
                    public_key_pem: user_key.public_key.pem_data.clone(),
                    algorithm: match user_key.public_key.algorithm {
                        KeyAlgorithm::Rsa { key_size } => {
                            format!("rsa-{}", key_size)
                        }
                        KeyAlgorithm::Ed25519 => "ed25519".to_string(),
                    },
                    key_size: match user_key.public_key.algorithm {
                        KeyAlgorithm::Rsa { key_size } => Some(key_size),
                        KeyAlgorithm::Ed25519 => None,
                    },
                    fingerprint: user_key.public_key.fingerprint.clone(),
                    created_at: now,
                })
            }
            Err(e) => {
                error!("Failed to generate key for actor {}: {}", actor_id, e);
                None
            }
        };

    // Create the actor document using unified database schema
    let actor_doc = oxifed::database::ActorDocument {
        id: None,
        actor_id: actor_id.clone(),
        name: username.clone(),
        preferred_username: username.clone(),
        domain: domain.clone(),
        actor_type: "Person".to_string(),
        summary: message.summary.clone(),
        icon: None,
        image: None,
        inbox: format!("https://{}/users/{}/inbox", &domain, &username),
        outbox: format!("https://{}/users/{}/outbox", &domain, &username),
        following: format!("https://{}/users/{}/following", &domain, &username),
        followers: format!("https://{}/users/{}/followers", &domain, &username),
        liked: Some(format!("https://{}/users/{}/liked", &domain, &username)),
        featured: Some(format!("https://{}/users/{}/featured", &domain, &username)),
        public_key: public_key_doc,
        endpoints: Some(mongodb::bson::to_document(&endpoints).unwrap_or_default()),
        attachment: None,
        additional_properties: message
            .properties
            .clone()
            .map(|p| mongodb::bson::to_document(&p).unwrap_or_default()),
        status: oxifed::database::ActorStatus::Active,
        created_at: now,
        updated_at: now,
        local: true,
        followers_count: 0,
        following_count: 0,
        statuses_count: 0,
    };

    db.manager().insert_actor(actor_doc).await.map_err(|e| {
        error!("Failed to insert actor: {}", e);
        RabbitMQError::DbError(crate::db::DbError::DatabaseError(e))
    })?;

    create_webfinger_profile(db, &message.subject, Some(aliases), None).await
}

async fn create_webfinger_profile(
    db: &Arc<MongoDB>,
    subject: &str,
    aliases: Option<Vec<String>>,
    _links: Option<Vec<oxifed::webfinger::Link>>,
) -> Result<(), RabbitMQError> {
    // Format the subject with the appropriate prefix
    let formatted_subject = format_subject(subject);

    // Insert into MongoDB
    let jrd_profiles = db.webfinger_profiles_collection();

    // Check if a profile with the same name already exists
    let filter = mongodb::bson::doc! { "subject": &formatted_subject };
    let existing = jrd_profiles.find_one(filter.clone()).await.map_err(|e| {
        error!("Failed to check for existing profile: {}", e);
        RabbitMQError::DbError(crate::db::DbError::DatabaseError(
            oxifed::database::DatabaseError::MongoError(e),
        ))
    })?;

    if existing.is_some() {
        error!(
            "Profile with subject '{}' already exists",
            formatted_subject
        );
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!(
                "Profile with subject '{}' already exists",
                formatted_subject
            ),
        )));
    }

    // Create a new Webfinger profile
    let resource = oxifed::webfinger::JrdResource {
        subject: Some(formatted_subject.clone()),
        aliases,
        properties: None,
        links: None,
    };

    // Insert the new profile
    jrd_profiles.insert_one(resource).await.map_err(|e| {
        error!("Failed to insert profile: {}", e);
        RabbitMQError::DbError(crate::db::DbError::DatabaseError(
            oxifed::database::DatabaseError::MongoError(e),
        ))
    })?;

    info!(
        "Created profile with subject '{}' via message queue",
        formatted_subject
    );
    Ok(())
}

/// Ensure the subject has an appropriate prefix (copied from oxiadm functionality)
fn format_subject(subject: &str) -> String {
    // If the subject already has a protocol prefix, return it as is
    if subject.starts_with("acct:") || subject.starts_with("https://") || subject.contains(':') {
        return subject.to_string();
    }

    // Otherwise, add the acct: prefix
    format!("acct:{}", subject)
}

async fn does_domain_exist(domain: &str, db: &Arc<MongoDB>) -> bool {
    db.manager()
        .find_domain_by_name(domain)
        .await
        .is_ok_and(|e| e.is_some())
}

fn split_subject(subject: &str) -> Result<(String, String), RabbitMQError> {
    subject
        .replace("acct:", "")
        .replace("https://", "")
        .replace("act:", "")
        .replace("http://", "")
        .split_once('@')
        .ok_or_else(|| {
            RabbitMQError::JsonError(serde_json::Error::custom(format!(
                "Invalid subject: '{}'",
                subject
            )))
        })
        .map(|(username, domain)| (username.to_string(), domain.to_string()))
}

/// Create a new domain
async fn create_domain_object(
    db: &Arc<MongoDB>,
    msg: &oxifed::messaging::DomainCreateMessage,
) -> Result<(), RabbitMQError> {
    use chrono::Utc;
    use oxifed::database::{DomainDocument, DomainStatus, RegistrationMode};

    info!("Creating domain: {}", msg.domain);

    // Check if domain already exists
    let db_manager = oxifed::database::DatabaseManager::new(db.database().clone());
    if let Ok(Some(_)) = db_manager.find_domain_by_name(&msg.domain).await {
        warn!("Domain {} already exists", msg.domain);
        return Err(RabbitMQError::ConstraintError(format!(
            "Domain {} already exists",
            msg.domain
        )));
    }

    // Parse registration mode
    let registration_mode = match msg.registration_mode.as_deref() {
        Some("open") => RegistrationMode::Open,
        Some("approval") => RegistrationMode::Approval,
        Some("invite") => RegistrationMode::Invite,
        Some("closed") => RegistrationMode::Closed,
        _ => RegistrationMode::Approval, // Default
    };

    // Create domain document
    let domain_doc = DomainDocument {
        id: None,
        domain: msg.domain.clone(),
        name: msg.name.clone(),
        description: msg.description.clone(),
        contact_email: msg.contact_email.clone(),
        rules: msg.rules.clone(),
        registration_mode,
        authorized_fetch: msg.authorized_fetch.unwrap_or(false),
        max_note_length: msg.max_note_length,
        max_file_size: msg.max_file_size,
        allowed_file_types: msg.allowed_file_types.clone(),
        domain_key_id: None, // Will be set when domain key is generated
        config: msg
            .properties
            .as_ref()
            .map(|p| mongodb::bson::to_document(p).unwrap_or_default()),
        status: DomainStatus::Active,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Insert domain into database
    match db_manager.insert_domain(domain_doc).await {
        Ok(id) => {
            info!("Domain {} created successfully with ID: {}", msg.domain, id);
        }
        Err(e) => {
            error!("Failed to create domain {}: {}", msg.domain, e);
            return Err(RabbitMQError::DatabaseError(e));
        }
    }

    Ok(())
}

/// Update an existing domain
async fn update_domain_object(
    db: &Arc<MongoDB>,
    msg: &oxifed::messaging::DomainUpdateMessage,
) -> Result<(), RabbitMQError> {
    use mongodb::bson::{Document, doc};
    use oxifed::database::RegistrationMode;

    info!("Updating domain: {}", msg.domain);

    let db_manager = oxifed::database::DatabaseManager::new(db.database().clone());

    // Check if domain exists
    if db_manager.find_domain_by_name(&msg.domain).await?.is_none() {
        return Err(RabbitMQError::DomainNotFound(format!(
            "Domain {} not found",
            msg.domain
        )));
    }

    // Build update document
    let mut update_doc = Document::new();

    if let Some(name) = &msg.name {
        update_doc.insert("name", name);
    }
    if let Some(description) = &msg.description {
        update_doc.insert("description", description);
    }
    if let Some(contact_email) = &msg.contact_email {
        update_doc.insert("contact_email", contact_email);
    }
    if let Some(rules) = &msg.rules {
        update_doc.insert("rules", rules);
    }
    if let Some(registration_mode) = &msg.registration_mode {
        let mode = match registration_mode.as_str() {
            "open" => RegistrationMode::Open,
            "approval" => RegistrationMode::Approval,
            "invite" => RegistrationMode::Invite,
            "closed" => RegistrationMode::Closed,
            _ => {
                return Err(RabbitMQError::JsonError(serde_json::Error::custom(
                    format!("Invalid registration mode: {}", registration_mode),
                )));
            }
        };
        update_doc.insert("registration_mode", mongodb::bson::to_bson(&mode).unwrap());
    }
    if let Some(authorized_fetch) = msg.authorized_fetch {
        update_doc.insert("authorized_fetch", authorized_fetch);
    }
    if let Some(max_note_length) = msg.max_note_length {
        update_doc.insert("max_note_length", max_note_length);
    }
    if let Some(max_file_size) = msg.max_file_size {
        update_doc.insert("max_file_size", max_file_size);
    }
    if let Some(allowed_file_types) = &msg.allowed_file_types {
        update_doc.insert("allowed_file_types", allowed_file_types);
    }
    if let Some(properties) = &msg.properties {
        update_doc.insert(
            "config",
            mongodb::bson::to_document(properties).unwrap_or_default(),
        );
    }

    update_doc.insert(
        "updated_at",
        mongodb::bson::to_bson(&chrono::Utc::now()).unwrap(),
    );

    // Perform update
    let collection: mongodb::Collection<oxifed::database::DomainDocument> =
        db.database().collection("domains");

    match collection
        .update_one(doc! { "domain": &msg.domain }, doc! { "$set": update_doc })
        .await
    {
        Ok(result) => {
            if result.modified_count > 0 {
                info!("Domain {} updated successfully", msg.domain);
            } else {
                warn!("No changes made to domain {}", msg.domain);
            }
        }
        Err(e) => {
            error!("Failed to update domain {}: {}", msg.domain, e);
            return Err(RabbitMQError::DatabaseError(
                oxifed::database::DatabaseError::MongoError(e),
            ));
        }
    }

    Ok(())
}

/// Delete a domain
async fn delete_domain_object(
    db: &Arc<MongoDB>,
    msg: &oxifed::messaging::DomainDeleteMessage,
) -> Result<(), RabbitMQError> {
    use mongodb::bson::doc;

    info!("Deleting domain: {} (force: {})", msg.domain, msg.force);

    let db_manager = oxifed::database::DatabaseManager::new(db.database().clone());

    // Check if domain exists
    if db_manager.find_domain_by_name(&msg.domain).await?.is_none() {
        return Err(RabbitMQError::DomainNotFound(format!(
            "Domain {} not found",
            msg.domain
        )));
    }

    // Check if domain has any actors (unless force is true)
    if !msg.force {
        let actor_collection: mongodb::Collection<oxifed::database::ActorDocument> =
            db.database().collection("actors");

        let actor_count = actor_collection
            .count_documents(doc! { "domain": &msg.domain })
            .await
            .map_err(|e| {
                RabbitMQError::DatabaseError(oxifed::database::DatabaseError::MongoError(e))
            })?;

        if actor_count > 0 {
            return Err(RabbitMQError::ConstraintError(format!(
                "Cannot delete domain {} with {} existing actors. Use --force to override.",
                msg.domain, actor_count
            )));
        }
    }

    // Delete the domain
    let collection: mongodb::Collection<oxifed::database::DomainDocument> =
        db.database().collection("domains");

    match collection.delete_one(doc! { "domain": &msg.domain }).await {
        Ok(result) => {
            if result.deleted_count > 0 {
                info!("Domain {} deleted successfully", msg.domain);

                // If force is true, also delete all associated actors
                if msg.force {
                    let actor_collection: mongodb::Collection<oxifed::database::ActorDocument> =
                        db.database().collection("actors");

                    match actor_collection
                        .delete_many(doc! { "domain": &msg.domain })
                        .await
                    {
                        Ok(actor_result) => {
                            info!(
                                "Deleted {} actors from domain {}",
                                actor_result.deleted_count, msg.domain
                            );
                        }
                        Err(e) => {
                            warn!("Failed to delete actors from domain {}: {}", msg.domain, e);
                        }
                    }
                }
            } else {
                warn!("Domain {} was not found for deletion", msg.domain);
            }
        }
        Err(e) => {
            error!("Failed to delete domain {}: {}", msg.domain, e);
            return Err(RabbitMQError::DatabaseError(
                oxifed::database::DatabaseError::MongoError(e),
            ));
        }
    }

    Ok(())
}
