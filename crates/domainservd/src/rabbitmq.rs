//! RabbitMQ/LavinMQ connection and message handling

use deadpool_lapin::{Config, Pool, Runtime};
use futures::StreamExt;
use lapin::{
    options::{
        BasicAckOptions, BasicConsumeOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::FieldTable, ExchangeKind,
};
use std::sync::Arc;
use serde::de::Error;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use oxifed::messaging::ProfileCreateMessage;
use crate::db::MongoDB;

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
    
    #[error("Profile not found: {0}")]
    ProfileNotFound(String),
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

    // Declare the create exchange - fanout style for broadcasting messages
    channel
        .exchange_declare(
            "oxifed.create",
            ExchangeKind::Fanout,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare the queue specific to domainservd for create operations
    channel
        .queue_declare(
            "domainservd.oxifed.create",
            QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Bind the queue to the create exchange
    channel
        .queue_bind(
            "domainservd.oxifed.create",
            "oxifed.create",
            "", // routing key not needed for fanout exchanges
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;
        
    // Declare the edit exchange - fanout style for broadcasting messages
    channel
        .exchange_declare(
            "oxifed.edit",
            ExchangeKind::Fanout,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Declare the queue specific to domainservd for edit operations
    channel
        .queue_declare(
            "domainservd.oxifed.edit",
            QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // Bind the queue to the edit exchange
    channel
        .queue_bind(
            "domainservd.oxifed.edit",
            "oxifed.edit",
            "", // routing key not needed for fanout exchanges
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    info!("RabbitMQ exchanges and queues initialized successfully");
    Ok(())
}

/// Start consumers for oxifed.create and oxifed.edit messages
pub async fn start_consumer(pool: Pool, db: Arc<MongoDB>) -> Result<(), RabbitMQError> {
    // Start create message consumer
    start_create_consumer(pool.clone(), db.clone()).await?;
    
    // Start edit message consumer
    start_edit_consumer(pool, db).await?;
    
    Ok(())
}

/// Start a consumer for oxifed.create messages
async fn start_create_consumer(pool: Pool, db: Arc<MongoDB>) -> Result<(), RabbitMQError> {
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;
    
    info!("Starting consumer for domainservd.oxifed.create queue");
    
    let mut consumer = channel
        .basic_consume(
            "domainservd.oxifed.create",
            "domainservd-create-consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // Process messages in a separate task
    tokio::spawn(async move {
        info!("Create consumer ready, waiting for messages");
        
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    match process_create_message(&delivery.data, &db).await {
                        Ok(_) => {
                            debug!("Successfully processed create message");
                            // Acknowledge the message
                            if let Err(e) = delivery
                                .ack(BasicAckOptions::default())
                                .await
                            {
                                error!("Failed to acknowledge create message: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to process create message: {}", e);
                            // Still acknowledge to avoid re-processing failed messages
                            if let Err(e) = delivery
                                .ack(BasicAckOptions::default())
                                .await
                            {
                                error!("Failed to acknowledge create message: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to consume create message: {}", e);
                }
            }
        }
        
        warn!("Create consumer stopped unexpectedly");
    });

    Ok(())
}

/// Start a consumer for oxifed.edit messages
async fn start_edit_consumer(pool: Pool, db: Arc<MongoDB>) -> Result<(), RabbitMQError> {
    let conn = pool.get().await?;
    let channel = conn.create_channel().await?;
    
    info!("Starting consumer for domainservd.oxifed.edit queue");
    
    let mut consumer = channel
        .basic_consume(
            "domainservd.oxifed.edit",
            "domainservd-edit-consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // Process messages in a separate task
    tokio::spawn(async move {
        info!("Edit consumer ready, waiting for messages");
        
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    match process_edit_message(&delivery.data, &db).await {
                        Ok(_) => {
                            debug!("Successfully processed edit message");
                            // Acknowledge the message
                            if let Err(e) = delivery
                                .ack(BasicAckOptions::default())
                                .await
                            {
                                error!("Failed to acknowledge edit message: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Failed to process edit message: {}", e);
                            // Still acknowledge to avoid re-processing failed messages
                            if let Err(e) = delivery
                                .ack(BasicAckOptions::default())
                                .await
                            {
                                error!("Failed to acknowledge edit message: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to consume edit message: {}", e);
                }
            }
        }
        
        warn!("Edit consumer stopped unexpectedly");
    });

    Ok(())
}

/// Process a profile creation message
async fn process_create_message(data: &[u8], db: &MongoDB) -> Result<(), RabbitMQError> {
    // Parse the message
    let message: ProfileCreateMessage = serde_json::from_slice(data)?;
    
    // Format the subject with appropriate prefix
    let formatted_subject = format_subject(&message.subject);
    
    // Create new profile
    let mut resource = oxifed::webfinger::JrdResource {
        subject: Some(formatted_subject.clone()),
        aliases: None,
        properties: Some(std::collections::HashMap::from([
            ("name".to_string(), serde_json::to_value(&message.name)?),
        ])),
        links: None,
    };

    // Process aliases if provided
    if let Some(aliases_str) = &message.aliases {
        let aliases_vec: Vec<String> = aliases_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        if !aliases_vec.is_empty() {
            resource.aliases = Some(aliases_vec);
        }
    }

    // Process links if provided
    if let Some(links_str) = &message.links {
        let links_vec = parse_links(links_str)?;
        if !links_vec.is_empty() {
            resource.links = Some(links_vec);
        }
    }

    // Insert into MongoDB
    let profiles = db.profiles_collection();
    
    // Check if a profile with the same name already exists
    let filter = mongodb::bson::doc! { "subject": &formatted_subject };
    let existing = profiles.find_one(filter.clone()).await.map_err(|e| {
        error!("Failed to check for existing profile: {}", e);
        RabbitMQError::DbError(crate::db::DbError::MongoError(e))
    })?;

    if existing.is_some() {
        error!("Profile with subject '{}' already exists", formatted_subject);
        return Err(RabbitMQError::DbError(crate::db::DbError::MongoError(
            mongodb::error::Error::custom(format!("Profile with subject '{}' already exists", 
                formatted_subject))
        )));
    }

    // Insert the new profile
    profiles.insert_one(resource).await.map_err(|e| {
        error!("Failed to insert profile: {}", e);
        RabbitMQError::DbError(crate::db::DbError::MongoError(e))   
    })?;
    
    info!("Created profile with subject '{}' via message queue", formatted_subject);
    Ok(())
}

/// Process a profile edit message
async fn process_edit_message(data: &[u8], db: &MongoDB) -> Result<(), RabbitMQError> {
    // Parse the message
    let message: oxifed::messaging::ProfileEditMessage = serde_json::from_slice(data)?;
    
    // Format the subject with appropriate prefix
    let formatted_subject = format_subject(&message.subject);
    
    let profiles = db.profiles_collection();
    
    // Find the existing profile
    let filter = mongodb::bson::doc! { "subject": &formatted_subject };
    let existing = profiles.find_one(filter.clone()).await.map_err(|e| {
        error!("Failed to check for existing profile: {}", e);
        RabbitMQError::DbError(crate::db::DbError::MongoError(e))
    })?;

    let mut resource = existing.ok_or_else(|| {
        error!("Profile with subject '{}' not found", formatted_subject);
        RabbitMQError::ProfileNotFound(formatted_subject.clone())
    })?;

    // Process updates
    
    // Update subject if provided
    if let Some(new_subj) = &message.new_subject {
        let formatted_new_subject = format_subject(new_subj);
        resource.subject = Some(formatted_new_subject);
    }

    // Handle aliases
    if message.clear_aliases {
        resource.aliases = None;
    } else if let Some(aliases_str) = &message.aliases {
        let aliases_vec: Vec<String> = aliases_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        if !aliases_vec.is_empty() {
            resource.aliases = Some(aliases_vec);
        }
    }

    // Handle links
    if message.clear_links {
        resource.links = None;
    } else if let Some(links_str) = &message.links {
        let new_links = parse_links(links_str)?;
        
        if !new_links.is_empty() {
            if let Some(existing_links) = &mut resource.links {
                // Add new links to existing ones
                existing_links.extend(new_links);
            } else {
                // Set new links
                resource.links = Some(new_links);
            }
        }
    }

    // Update in MongoDB
    // If subject was changed, we need to delete the old document and insert a new one
    if message.new_subject.is_some() {
        // Delete the old document
        profiles.delete_one(filter.clone()).await.map_err(|e| {
            error!("Failed to delete old profile: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;
        
        // Insert the updated profile as a new document
        profiles.insert_one(resource.clone()).await.map_err(|e| {
            error!("Failed to insert updated profile: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;
    } else {
        // Just update the existing document
        profiles.replace_one(filter, resource.clone()).await.map_err(|e| {
            error!("Failed to update profile: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;
    }
    
    info!("Updated profile with subject '{}' via message queue", formatted_subject);
    if let Some(new_subj) = &message.new_subject {
        let formatted_new_subject = format_subject(new_subj);
        info!("Subject changed to '{}'", formatted_new_subject);
    }
    
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

/// Parse links from a string (copied from oxiadm functionality)
fn parse_links(links_str: &str) -> Result<Vec<oxifed::webfinger::Link>, RabbitMQError> {
    let mut result = Vec::new();

    for link_str in links_str.split(';') {
        let parts: Vec<&str> = link_str.split(',').collect();
        if parts.len() < 2 {
            return Err(RabbitMQError::JsonError(
                serde_json::Error::custom(format!("Invalid link format: '{}'", link_str))
            ));
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
