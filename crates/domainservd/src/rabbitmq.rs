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
use oxifed::messaging::{
    AnnounceActivityMessage, FollowActivityMessage, LikeActivityMessage, MessageEnum,
    NoteCreateMessage, NoteDeleteMessage, NoteUpdateMessage, ProfileCreateMessage,
    ProfileDeleteMessage, ProfileUpdateMessage,
};
use serde::de::Error;
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

    // Declare the publish exchange for ActivityPub messages
    channel
        .exchange_declare(
            "oxifed.publish",
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
            "domainservd.oxifed.activities",
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
                    match process_message(&delivery.data, &db, &channel).await {
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
async fn process_message(
    data: &[u8],
    db: &MongoDB,
    channel: &lapin::Channel,
) -> Result<(), RabbitMQError> {
    // Parse the message
    let message: MessageEnum = serde_json::from_slice(data)?;

    match message {
        MessageEnum::ProfileCreateMessage(msg) => create_person_object(db, &msg).await,
        MessageEnum::ProfileUpdateMessage(msg) => update_person_object(db, &msg).await,
        MessageEnum::ProfileDeleteMessage(msg) => delete_person_object(db, &msg).await,
        MessageEnum::NoteCreateMessage(msg) => create_note_object(db, channel, &msg).await,
        MessageEnum::NoteUpdateMessage(msg) => update_note_object(db, channel, &msg).await,
        MessageEnum::NoteDeleteMessage(msg) => delete_note_object(db, channel, &msg).await,
        MessageEnum::FollowActivityMessage(msg) => handle_follow(db, &msg).await,
        MessageEnum::LikeActivityMessage(msg) => handle_like(db, &msg).await,
        MessageEnum::AnnounceActivityMessage(msg) => handle_announce(db, &msg).await,
    }
}

async fn handle_announce(
    _db: &MongoDB,
    _msg: &AnnounceActivityMessage,
) -> Result<(), RabbitMQError> {
    todo!()
}

async fn handle_like(_db: &MongoDB, _msg: &LikeActivityMessage) -> Result<(), RabbitMQError> {
    todo!()
}

async fn handle_follow(_db: &MongoDB, _msg: &FollowActivityMessage) -> Result<(), RabbitMQError> {
    todo!()
}

// Stub implementations for unimplemented functions
async fn handle_activity(_db: &MongoDB, _msg: &str) -> Result<(), RabbitMQError> {
    todo!()
}

// Helper function to publish an activity to the oxifed.activities exchange
async fn publish_activity_to_followers(
    activity: &oxifed::Activity,
    channel: &lapin::Channel,
) -> Result<(), RabbitMQError> {
    // Convert the activity to JSON for publishing
    let activity_json = serde_json::to_vec(activity)?;

    // Publish to the oxifed.publish exchange
    channel
        .basic_publish(
            "oxifed.publish",
            "", // no routing key for fanout exchanges
            lapin::options::BasicPublishOptions::default(),
            &activity_json,
            lapin::BasicProperties::default(),
        )
        .await?;

    Ok(())
}

async fn delete_note_object(
    db: &MongoDB,
    channel: &lapin::Channel,
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
    let outbox_collection = db.outbox_collection(username);
    let filter = mongodb::bson::doc! { "id": &msg.id };

    let note = if !msg.force {
        // If not using force delete, we need the note details for the Delete activity
        outbox_collection
            .find_one(filter.clone())
            .await
            .map_err(|e| {
                error!("Failed to find note: {}", e);
                RabbitMQError::DbError(crate::db::DbError::MongoError(e))
            })?
    } else {
        None
    };

    // If the note doesn't exist and we're not forcing deletion, return an error
    if note.is_none() && !msg.force {
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!("Note not found: {}", msg.id),
        )));
    }

    // Delete the note
    outbox_collection.delete_one(filter).await.map_err(|e| {
        error!("Failed to delete note: {}", e);
        RabbitMQError::DbError(crate::db::DbError::MongoError(e))
    })?;

    // Create a tombstone if a note was found
    if let Some(note_obj) = note {
        // Create a tombstone object to replace the note
        let now = chrono::Utc::now();

        // Create the actor ID URL
        let actor_id_url = url::Url::parse(&format!("https://{}/u/{}", domain, username))
            .map_err(|e| RabbitMQError::URLParse(e))?;

        // Create the activity ID URL
        let activity_id_url =
            url::Url::parse(&format!("{}/delete/{}", msg.id, now.timestamp_millis()))
                .map_err(|e| RabbitMQError::URLParse(e))?;

        // Create a Delete activity
        let activity = oxifed::Activity {
            activity_type: oxifed::ActivityType::Delete,
            id: Some(activity_id_url),
            name: None,
            summary: None,
            actor: Some(oxifed::ObjectOrLink::Url(actor_id_url)),
            object: Some(oxifed::ObjectOrLink::Object(Box::new(note_obj))),
            target: None,
            published: Some(now),
            updated: Some(now),
            additional_properties: std::collections::HashMap::new(),
        };

        // Insert the activity into activities collection
        let activities_collection = db.activities_collection(username);
        activities_collection
            .insert_one(&activity)
            .await
            .map_err(|e| {
                error!("Failed to insert activity: {}", e);
                RabbitMQError::DbError(crate::db::DbError::MongoError(e))
            })?;

        // Publish the activity to followers
        publish_activity_to_followers(&activity, &channel).await?;
    }

    info!("Note deleted successfully: {}", msg.id);
    Ok(())
}

async fn update_note_object(
    db: &MongoDB,
    channel: &lapin::Channel,
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
    let outbox_collection = db.outbox_collection(username);
    let note_id_url = url::Url::parse(&msg.id).map_err(|e| RabbitMQError::URLParse(e))?;
    let filter = mongodb::bson::doc! { "id": &note_id_url.to_string() };

    let note = outbox_collection.find_one(filter.clone()).await.map_err(|e| {
        error!("Failed to find note: {}", e);
        RabbitMQError::DbError(crate::db::DbError::MongoError(e))
    })?;

    let mut note = note.ok_or_else(|| {
        RabbitMQError::JsonError(serde_json::Error::custom(format!(
            "Note not found: {}",
            msg.id
        )))
    })?;

    let now = chrono::Utc::now();

    // Update note fields if provided
    let mut update_doc = mongodb::bson::Document::new();
    let mut set_doc = mongodb::bson::Document::new();

    if let Some(content) = &msg.content {
        note.content = Some(content.clone());
        set_doc.insert("content", content);
    }

    if let Some(summary) = &msg.summary {
        note.summary = Some(summary.clone());
        set_doc.insert("summary", summary);
    }

    // Update tags if provided
    if let Some(tags_str) = &msg.tags {
        let tags: Vec<&str> = tags_str.split(',').map(|s| s.trim()).collect();
        let tags_value = serde_json::Value::Array(
            tags.into_iter()
                .map(|t| serde_json::Value::String(t.to_string()))
                .collect(),
        );

        note.additional_properties
            .insert("tag".to_string(), tags_value.clone());
        set_doc.insert(
            "additional_properties.tag",
            mongodb::bson::to_bson(&tags_value)?,
        );
    }

    // Update custom properties if provided
    if let Some(props) = &msg.properties {
        for (k, v) in props.as_object().unwrap_or(&serde_json::Map::new()) {
            note.additional_properties.insert(k.clone(), v.clone());
            set_doc.insert(
                format!("additional_properties.{}", k),
                mongodb::bson::to_bson(v)?,
            );
        }
    }

    // Always update the 'updated' timestamp
    note.updated = Some(now);
    set_doc.insert("updated", mongodb::bson::to_bson(&now)?);

    update_doc.insert("$set", set_doc);

    // Update the note in the database
    outbox_collection
        .update_one(filter, update_doc)
        .await
        .map_err(|e| {
            error!("Failed to update note: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;

    // Create the actor ID URL
    let actor_id_url = url::Url::parse(&format!("https://{}/u/{}", domain, username))
        .map_err(|e| RabbitMQError::URLParse(e))?;

    // Create the activity ID URL
    let activity_id_url = url::Url::parse(&format!("{}/update/{}", msg.id, now.timestamp_millis()))
        .map_err(|e| RabbitMQError::URLParse(e))?;

    // Create an Update activity
    let activity = oxifed::Activity {
        activity_type: oxifed::ActivityType::Update,
        id: Some(activity_id_url),
        name: None,
        summary: None,
        actor: Some(oxifed::ObjectOrLink::Url(actor_id_url)),
        object: Some(oxifed::ObjectOrLink::Object(Box::new(note))),
        target: None,
        published: Some(now),
        updated: Some(now),
        additional_properties: std::collections::HashMap::new(),
    };

    // Insert the activity into activities collection
    let activities_collection = db.activities_collection(username);
    activities_collection
        .insert_one(&activity)
        .await
        .map_err(|e| {
            error!("Failed to insert activity: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;

    // Publish the activity to followers
    publish_activity_to_followers(&activity, &channel).await?;

    info!("Note updated successfully: {}", msg.id);
    Ok(())
}

async fn create_note_object(
    db: &MongoDB,
    channel: &lapin::Channel,
    msg: &NoteCreateMessage,
) -> Result<(), RabbitMQError> {
    // Parse username and domain from author
    let (username, domain) = split_subject(&msg.author)?;

    if !does_domain_exist(&domain, db).await {
        return Err(RabbitMQError::DomainNotFound(domain));
    }

    // Get the actor to attach as attributedTo
    let actor_collection = db.actors_collection();
    let actor_id_str = format!("https://{}/u/{}", &domain, &username);
    let filter = mongodb::bson::doc! { "id": &actor_id_str };

    let actor = actor_collection
        .find_one(filter.clone())
        .await
        .map_err(|e| {
            error!("Failed to find actor: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;

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
    let note_id_url = url::Url::parse(&note_id).map_err(|e| RabbitMQError::URLParse(e))?;
    
    // Check if a note with this ID already exists
    let outbox_collection = db.outbox_collection(&username);
    let existing_note = outbox_collection
        .find_one(mongodb::bson::doc! { "id": &note_id })
        .await
        .map_err(|e| {
            error!("Failed to check for existing note: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;
        
    if existing_note.is_some() {
        error!("Note with ID '{}' already exists", note_id);
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!("Note with ID '{}' already exists", note_id)
        )));
    }

    // Parse the actor ID into a URL
    let actor_id_url = url::Url::parse(&actor_id_str).map_err(|e| RabbitMQError::URLParse(e))?;

    let now = chrono::Utc::now();

    // Create the note object
    let mut note = oxifed::Object {
        object_type: oxifed::ObjectType::Note,

        id: Some(note_id_url.clone()),
        name: None,
        summary: msg.summary.clone(),
        content: Some(msg.content.clone()),
        url: Some(note_id_url.clone()),
        published: Some(now),
        updated: Some(now),
        attributed_to: Some(oxifed::ObjectOrLink::Url(actor_id_url.clone())),
        additional_properties: std::collections::HashMap::new(),
    };

    // Add tags if provided
    if let Some(tags_str) = &msg.tags {
        let tags: Vec<&str> = tags_str.split(',').map(|s| s.trim()).collect();
        if !tags.is_empty() {
            note.additional_properties.insert(
                "tags".to_string(),
                serde_json::Value::Array(
                    tags.into_iter()
                        .map(|t| serde_json::Value::String(t.to_string()))
                        .collect(),
                ),
            );
        }
    }

    // Add mentions if provided
    if let Some(mentions_str) = &msg.mentions {
        let mentions: Vec<&str> = mentions_str.split(',').map(|s| s.trim()).collect();
        if !mentions.is_empty() {
            note.additional_properties.insert(
                "mentions".to_string(),
                serde_json::Value::Array(
                    mentions
                        .into_iter()
                        .map(|m| serde_json::Value::String(m.to_string()))
                        .collect(),
                ),
            );
        }
    }

    // Add any custom properties
    if let Some(props) = &msg.properties {
        for (k, v) in props.as_object().unwrap_or(&serde_json::Map::new()) {
            note.additional_properties.insert(k.clone(), v.clone());
        }
    }

    // Insert the note into the outbox collection
    outbox_collection
        .insert_one(note.clone())
        .await
        .map_err(|e| {
            error!("Failed to insert note into outbox: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;

    // Create activity ID
    let activity_id_url = url::Url::parse(&format!("{}/activity", note_id))
        .map_err(|e| RabbitMQError::URLParse(e))?;

    // Create a Create activity
    let activity = oxifed::Activity {
        activity_type: oxifed::ActivityType::Create,
        id: Some(activity_id_url),
        name: None,
        summary: None,
        actor: Some(oxifed::ObjectOrLink::Url(actor_id_url)),
        object: Some(oxifed::ObjectOrLink::Object(Box::new(note))),
        target: None,
        published: Some(now),
        updated: Some(now),
        additional_properties: std::collections::HashMap::new(),
    };

    // Insert the activity into activities collection
    let activities_collection = db.activities_collection(&username);
    activities_collection
        .insert_one(&activity)
        .await
        .map_err(|e| {
            error!("Failed to insert activity: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;

    // Publish the activity to followers
    publish_activity_to_followers(&activity, channel).await?;

    info!("Note created successfully: {}", note_id);
    Ok(())
}

async fn delete_person_object(
    _db: &MongoDB,
    msg: &ProfileDeleteMessage,
) -> Result<(), RabbitMQError> {
    // Just a stub for now, will implement later
    info!("Delete person request received for ID: {}", msg.id);
    Ok(())
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
    let mut set_doc = mongodb::bson::doc! {};

    if let Some(summary) = &msg.summary {
        set_doc.insert("summary", summary);
    }

    if let Some(icon) = &msg.icon {
        set_doc.insert("icon", bson::to_bson(&icon)?);
    }

    if let Some(attachments) = &msg.attachments {
        set_doc.insert("attachment", bson::to_bson(&attachments)?);
    }

    if !set_doc.is_empty() {
        update.insert("$set", set_doc);
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

    // Create the actor ID and check if it already exists
    let actor_id = format!("https://{}/u/{}", &domain, &username);
    let actor_collection = db.actors_collection();
    
    // Check if an actor with this ID already exists
    let existing_actor = actor_collection
        .find_one(mongodb::bson::doc! { "id": &actor_id })
        .await
        .map_err(|e| {
            error!("Failed to check for existing actor: {}", e);
            RabbitMQError::DbError(crate::db::DbError::MongoError(e))
        })?;
        
    if existing_actor.is_some() {
        error!("Actor with ID '{}' already exists", actor_id);
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!("Actor with ID '{}' already exists", actor_id)
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

    // Create the actor/person object
    let person = oxifed::Actor {
        id: actor_id,
        name: username.clone(),
        domain: domain.clone(),
        inbox_url: format!("https://{}/u/{}/inbox", &domain, &username),
        outbox_url: format!("https://{}/u/{}/outbox", &domain, &username),
        following_url: Some(format!("https://{}/u/{}/following", &domain, &username)),
        followers_url: Some(format!("https://{}/u/{}/followers", &domain, &username)),
        created_at: now,
        updated_at: now,
        endpoints,
        icon: None,
        attachment: None,
    };

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
        return Err(RabbitMQError::JsonError(serde_json::Error::custom(
            format!("Profile with subject '{}' already exists", formatted_subject)
        )));
    }

    // Create a new Webfinger profile
    let resource = oxifed::webfinger::JrdResource {
        subject: Some(formatted_subject.clone()),
        aliases,
        properties: None,
        links,
    };

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
