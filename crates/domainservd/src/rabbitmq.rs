//! RabbitMQ/LavinMQ connection and message handling

use crate::db::MongoDB;
use deadpool_lapin::{Config, Pool, Runtime};
use futures::StreamExt;
use lapin::{
    ExchangeKind,
    options::{
        BasicAckOptions, BasicConsumeOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::FieldTable,
};
use mongodb::bson;
use oxifed::ObjectType;
use oxifed::messaging::{
    AnnounceActivityMessage, FollowActivityMessage, LikeActivityMessage, Message,
    NoteCreateMessage, NoteDeleteMessage, NoteUpdateMessage, ProfileCreateMessage,
    ProfileDeleteMessage, ProfileUpdateMessage,
};
use serde::de::Error;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};

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

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("Domain not found: {0}")]
    DomainNotFound(String),
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
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;

    // Declare the activities exchange - fanout style for broadcasting messages
    channel
        .exchange_declare(
            "oxifed.activities",
            ExchangeKind::Fanout,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare the edit exchange - fanout style for broadcasting messages
    channel
        .exchange_declare(
            "oxifed.activities",
            ExchangeKind::Fanout,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Bind the queue to the edit exchange
    channel
        .queue_bind(
            "domainservd.oxifed.activities",
            "oxifed.activities",
            "", // routing key not needed for fanout exchanges
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

    Ok(())
}

/// Start a consumer for oxifed.activities messages
async fn start_activities_consumer(pool: Pool, db: Arc<MongoDB>) -> Result<(), RabbitMQError> {
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;

    info!("Starting consumer for domainservd.oxifed.activities queue");

    let mut consumer = channel
        .basic_consume(
            "domainservd.oxifed.activities",
            "domainservd-activities-consumer",
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
                    match process_create_message(&delivery.data, &db).await {
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
async fn process_create_message(data: &[u8], db: &MongoDB) -> Result<(), RabbitMQError> {
    // Parse the message
    let message: Message = serde_json::from_slice(data)?;

    match message {
        Message::ProfileCreateMessage(msg) => create_person_object(db, &msg).await,
        Message::ProfileUpdateMessage(msg) => update_person_object(db, &msg).await,
        Message::ProfileDeleteMessage(msg) => delete_person_object(db, &msg).await,
        Message::NoteCreateMessage(msg) => create_note_object(db, &msg).await,
        Message::NoteUpdateMessage(msg) => update_note_object(db, &msg).await,
        Message::NoteDeleteMessage(msg) => delete_note_object(db, &msg).await,
        Message::FollowActivityMessage(msg) => handle_follow(db, &msg).await,
        Message::LikeActivityMessage(msg) => handle_like(db, &msg).await,
        Message::AnnounceActivityMessage(msg) => handle_announce(db, &msg).await,
    }
}

async fn handle_announce(db: &MongoDB, msg: &AnnounceActivityMessage) -> Result<(), RabbitMQError> {
    todo!()
}

async fn handle_like(db: &MongoDB, msg: &LikeActivityMessage) -> Result<(), RabbitMQError> {
    todo!()
}

async fn handle_follow(db: &MongoDB, msg: &FollowActivityMessage) -> Result<(), RabbitMQError> {
    todo!()
}

async fn delete_note_object(db: &MongoDB, msg: &NoteDeleteMessage) -> Result<(), RabbitMQError> {
    todo!()
}

async fn update_note_object(db: &MongoDB, msg: &NoteUpdateMessage) -> Result<(), RabbitMQError> {
    todo!()
}

async fn create_note_object(db: &MongoDB, msg: &NoteCreateMessage) -> Result<(), RabbitMQError> {
    todo!()
}

async fn delete_person_object(
    db: &MongoDB,
    msg: &ProfileDeleteMessage,
) -> Result<(), RabbitMQError> {
    todo!()
}

async fn update_person_object(
    db: &MongoDB,
    msg: &ProfileUpdateMessage,
) -> Result<(), RabbitMQError> {
    let (username, domain) = split_subject(&msg.subject)?;

    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain));
    }

    let actor_collection = db.actors_collection();

    let filter = mongodb::bson::doc! { "id": &format!("https://{}/u/{}", &domain, &username) };

    let mut update = mongodb::bson::doc! {};

    if let Some(summary) = &msg.summary {
        update.insert("$set", mongodb::bson::doc! { "summary": summary });
    }

    if let Some(icon) = &msg.icon {
        update.insert(
            "$set",
            mongodb::bson::doc! { "icon": bson::to_bson(&icon)? },
        );
    }

    if let Some(attachments) = &msg.attachments {
        update.insert(
            "$set",
            mongodb::bson::doc! {"attachment": bson::to_bson(&attachments)? },
        );
    }

    actor_collection
        .update_one(filter, update)
        .await
        .map_err(|e| {
            error!("Failed to update actor: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;

    Ok(())
}

async fn create_person_object(
    db: &MongoDB,
    message: &ProfileCreateMessage,
) -> Result<(), RabbitMQError> {
    let (username, domain) = split_subject(&message.subject)?;

    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain));
    }

    let aliases = vec![format!("https://{}/@{}", domain, username)];

    let person = oxifed::Actor {
        id: format!("https://{}/u/{}", &domain, &username),
        name: username.clone(),
        domain: domain.clone(),
        inbox_url: format!("https://{}/u/{}/inbox", &domain, &username),
        outbox_url: format!("https://{}/u/{}/outbox", &domain, &username),
        following_url: Some(format!("https://{}/u/{}/following", &domain, &username)),
        followers_url: Some(format!("https://{}/u/{}/followers", &domain, &username)),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        endpoints: HashMap::from([(
            "sharedInbox".to_string(),
            format!("https://{}/sharedInbox", &domain),
        )]),
        icon: None,
        attachment: None,
    };

    let actor_collection = db.actors_collection();

    actor_collection.insert_one(person).await.map_err(|e| {
        error!("Failed to insert actor: {}", e);
        RabbitMQError::DbError(crate::db::DbError::MongoError(e))
    })?;

    create_webfinger_profile(db, &message.subject, Some(aliases), None).await
}

async fn create_webfinger_profile(
    db: &MongoDB,
    subject: &str,
    aliases: Option<Vec<String>>,
    links: Option<Vec<oxifed::webfinger::Link>>,
) -> Result<(), RabbitMQError> {
    // Format the subject with the appropriate prefix
    let formatted_subject = format_subject(subject);

    // Create a new Webfinger profile
    let resource = oxifed::webfinger::JrdResource {
        subject: Some(formatted_subject.clone()),
        aliases,
        properties: None,
        links,
    };

    // Insert into MongoDB
    let jrd_profiles = db.webfinger_profiles_collection();

    // Check if a profile with the same name already exists
    let filter = mongodb::bson::doc! { "subject": &formatted_subject };
    let existing = jrd_profiles.find_one(filter.clone()).await.map_err(|e| {
        error!("Failed to check for existing profile: {}", e);
        RabbitMQError::DbError(crate::db::DbError::MongoError(e))
    })?;

    if existing.is_some() {
        error!(
            "Profile with subject '{}' already exists",
            formatted_subject
        );
        return Err(RabbitMQError::DbError(crate::db::DbError::MongoError(
            mongodb::error::Error::custom(format!(
                "Profile with subject '{}' already exists",
                formatted_subject
            )),
        )));
    }

    // Insert the new profile
    jrd_profiles.insert_one(resource).await.map_err(|e| {
        error!("Failed to insert profile: {}", e);
        RabbitMQError::DbError(crate::db::DbError::MongoError(e))
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

async fn does_domain_exist(domain: &str, db: &MongoDB) -> bool {
    let domains = db.domains_collection();
    let filter = mongodb::bson::doc! { "dnsName": &domain };
    domains.find_one(filter).await.is_ok_and(|e| e.is_some())
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

/// Parse links from a string (copied from oxiadm functionality)
fn parse_links(links_str: &str) -> Result<Vec<oxifed::webfinger::Link>, RabbitMQError> {
    let mut result = Vec::new();

    for link_str in links_str.split(';') {
        let parts: Vec<&str> = link_str.split(',').collect();
        if parts.len() < 2 {
            return Err(RabbitMQError::JsonError(serde_json::Error::custom(
                format!("Invalid link format: '{}'", link_str),
            )));
        }

        let rel = parts[0].trim().to_string();
        let href = parts[1].trim().to_string();

        let title = if parts.len() > 2 {
            Some(parts[2].trim().to_string())
        } else {
            None
        };

        let link = oxifed::webfinger::Link {
            rel,
            href: Some(href),
            type_: None,
            titles: title.map(|t| {
                let mut map = std::collections::HashMap::new();
                map.insert("en".to_string(), t);
                map
            }),
            properties: None,
        };

        result.push(link);
    }

    Ok(result)
}
