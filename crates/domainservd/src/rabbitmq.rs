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


use oxifed::messaging::{
    AcceptActivityMessage, AnnounceActivityMessage, FollowActivityMessage, LikeActivityMessage,
    MessageEnum, NoteCreateMessage, NoteDeleteMessage, NoteUpdateMessage, ProfileCreateMessage,
    ProfileDeleteMessage, ProfileUpdateMessage, RejectActivityMessage,
};
use std::time::SystemTime;
use mongodb::bson::Bson;
use oxifed::messaging::{EXCHANGE_ACTIVITYPUB_PUBLISH, EXCHANGE_INTERNAL_PUBLISH};
use serde::de::Error;
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Constants for RabbitMQ queue names
pub const QUEUE_ACTIVITIES: &str = "domainservd.oxifed.activities";
pub const CONSUMER_TAG: &str = "domainservd-activities-consumer";

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

    // Declare the activities queue
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
async fn handle_accept_activity(
    db: &Arc<MongoDB>,
    activity: &oxifed::Activity,
) -> Result<(), RabbitMQError> {
    // Check if this is accepting a Follow activity
    if let Some(oxifed::ObjectOrLink::Object(follow_obj)) = &activity.object {
        if follow_obj.object_type == oxifed::ObjectType::Activity {
            // This is accepting a Follow activity
            if let Some(follow_actor) = &follow_obj.additional_properties.get("actor") {
                if let Some(follow_target) = &follow_obj.additional_properties.get("object") {
                    if let (
                        serde_json::Value::String(follower_id),
                        serde_json::Value::String(target_id),
                    ) = (follow_actor, follow_target)
                    {
                        return add_follower_relationship(db, follower_id, target_id).await;
                    }
                }
            }
        }
    }

    info!("Accept activity processed (not a follow accept)");
    Ok(())
}

/// Add a follower relationship to the database
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
        .update_follow_status(follower_id, &target_actor_id, oxifed::database::FollowStatus::Cancelled)
        .await
        .map_err(|e| RabbitMQError::DbError(crate::db::DbError::DatabaseError(e)))?;

    info!(
        "Removed follower relationship: {} -> {}",
        follower_id, target_id
    );
    Ok(())
}

// ActivityPub-compliant delivery to followers according to W3C specification
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

    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain.to_string()));
    }

    // Find the note before deleting it (we need it for the Delete activity)
    // TODO: Use unified database manager for outbox operations
    let _filter = mongodb::bson::doc! { "id": &msg.id };

    let _note: Option<oxifed::database::ObjectDocument> = if !msg.force {
        // Use unified database manager to find note
        db.manager().find_object_by_id(&msg.id).await
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

    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain.to_string()));
    }

    // Find the note to update
    // Find the existing note
    let existing_note = db.manager()
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
        let tags_docs: Vec<mongodb::bson::Document> = tags.into_iter().map(|tag| {
            mongodb::bson::doc! {
                "tag_type": "Hashtag",
                "name": tag,
                "href": format!("https://{}/tags/{}", domain, tag)
            }
        }).collect();
        update_doc.insert("tag", tags_docs);
    }

    // Add custom properties if provided
    if let Some(props) = &msg.properties {
        let props_doc = mongodb::bson::to_document(props)
            .map_err(RabbitMQError::BsonError)?;
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

    let actor = db.find_actor_by_id(&actor_id_str)
        .await
        .map_err(|e| RabbitMQError::DbError(e))?;

    if actor.is_none() {
        return Err(RabbitMQError::ProfileNotFound(actor_id_str));
    }

    // Create a unique ID for this note
    let note_id_uuid = uuid::Uuid::new_v4();
    let note_id = format!(
        "https://{}/u/{}/notes/{}",
        &domain,
        &username,
        note_id_uuid.to_string()
    );

    // Parse the note ID into a URL
    let _note_id_url = url::Url::parse(&note_id).map_err(|e| RabbitMQError::URLParse(e))?;

    // Check if a note with this ID already exists
    let existing_note = db.manager()
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
    let _actor_id_url = url::Url::parse(&actor_id_str).map_err(|e| RabbitMQError::URLParse(e))?;

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
        additional_properties: msg.properties.clone().map(|p| mongodb::bson::to_document(&p).unwrap_or_default()),
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
    info!("Publishing activity {} to ActivityPub exchange", activity.activity_id);
    
    // Convert ActivityDocument to legacy Activity format for publishing
    let _legacy_activity = oxifed::Activity {
        activity_type: activity.activity_type.clone(),
        id: Some(url::Url::parse(&activity.activity_id).map_err(RabbitMQError::URLParse)?),
        name: activity.name.clone(),
        summary: activity.summary.clone(),
        actor: Some(oxifed::ObjectOrLink::Url(
            url::Url::parse(&activity.actor).map_err(RabbitMQError::URLParse)?
        )),
        object: activity.object.as_ref().map(|obj| {
            oxifed::ObjectOrLink::Url(
                url::Url::parse(obj).unwrap_or_else(|_| url::Url::parse("https://example.com/unknown").unwrap())
            )
        }),
        target: activity.target.as_ref().map(|t| {
            oxifed::ObjectOrLink::Url(
                url::Url::parse(t).unwrap_or_else(|_| url::Url::parse("https://example.com/unknown").unwrap())
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
        update_doc.insert("icon", mongodb::bson::to_bson(&icon).map_err(RabbitMQError::BsonError)?);
    }

    if let Some(attachments) = &msg.attachments {
        update_doc.insert("attachment", mongodb::bson::to_bson(&attachments).map_err(RabbitMQError::BsonError)?);
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
    let existing_actor = db.find_actor_by_id(&actor_id)
        .await
        .map_err(|e| RabbitMQError::DbError(e))?;

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
        public_key: None, // TODO: Generate public key
        endpoints: Some(mongodb::bson::to_document(&endpoints).unwrap_or_default()),
        attachment: None,
        additional_properties: message.properties.clone().map(|p| mongodb::bson::to_document(&p).unwrap_or_default()),
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
        RabbitMQError::DbError(crate::db::DbError::DatabaseError(oxifed::database::DatabaseError::MongoError(e)))
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
        RabbitMQError::DbError(crate::db::DbError::DatabaseError(oxifed::database::DatabaseError::MongoError(e)))
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
