//! Database Module for Oxifed
//!
//! Provides MongoDB schemas and operations for ActivityPub entities,
//! PKI key management, and system configuration.

use crate::pki::TrustLevel;
use crate::{ActivityType, ObjectType};
use chrono::{DateTime, Utc};
use futures::stream::TryStreamExt;
use mongodb::{
    Collection, Database, IndexModel,
    bson::{Bson, Document, doc, oid::ObjectId},
    error::Error as MongoError,
    options::IndexOptions,
    results::UpdateResult,
};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use thiserror::Error;

/// Database-related errors
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("MongoDB error: {0}")]
    MongoError(#[from] MongoError),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] mongodb::bson::ser::Error),

    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] mongodb::bson::de::Error),

    #[error("Document not found: {0}")]
    NotFoundError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Constraint violation: {0}")]
    ConstraintError(String),

    #[error("Operation failed: {0}")]
    OperationError(String),
}

/// Actor document in MongoDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Unique ActivityPub ID
    pub actor_id: String,

    /// Display name
    pub name: String,

    /// Preferred username (local part)
    pub preferred_username: String,

    /// Domain this actor belongs to
    pub domain: String,

    /// Actor type (Person, Service, etc.)
    pub actor_type: String,

    /// Profile summary/bio
    pub summary: Option<String>,

    /// Profile icon/avatar URL
    pub icon: Option<String>,

    /// Profile header image URL
    pub image: Option<String>,

    /// Inbox URL
    pub inbox: String,

    /// Outbox URL
    pub outbox: String,

    /// Following collection URL
    pub following: String,

    /// Followers collection URL
    pub followers: String,

    /// Liked collection URL
    pub liked: Option<String>,

    /// Featured collection URL
    pub featured: Option<String>,

    /// Public key information
    pub public_key: Option<PublicKeyDocument>,

    /// Additional endpoints
    pub endpoints: Option<Document>,

    /// Profile attachments (links, properties)
    pub attachment: Option<Vec<Document>>,

    /// Custom properties
    pub additional_properties: Option<Document>,

    /// Account status
    pub status: ActorStatus,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Local account flag
    pub local: bool,

    /// Follower count
    pub followers_count: i64,

    /// Following count
    pub following_count: i64,

    /// Status count
    pub statuses_count: i64,
}

/// Public key embedded document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKeyDocument {
    pub id: String,
    pub owner: String,
    pub public_key_pem: String,
    pub algorithm: String,
    pub key_size: Option<u32>,
    pub fingerprint: String,
    pub created_at: DateTime<Utc>,
}

/// Actor status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActorStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "suspended")]
    Suspended,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "pending")]
    Pending,
}

/// Object document in MongoDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// ActivityPub object ID
    pub object_id: String,

    /// Object type (Note, Article, etc.)
    pub object_type: ObjectType,

    /// Actor who created this object
    pub attributed_to: String,

    /// Content/body of the object
    pub content: Option<String>,

    /// Summary or excerpt
    pub summary: Option<String>,

    /// Display name
    pub name: Option<String>,

    /// Media type of content
    pub media_type: Option<String>,

    /// Object URL
    pub url: Option<String>,

    /// Published timestamp
    pub published: Option<DateTime<Utc>>,

    /// Updated timestamp
    pub updated: Option<DateTime<Utc>>,

    /// Addressing - to field
    pub to: Option<Vec<String>>,

    /// Addressing - cc field
    pub cc: Option<Vec<String>>,

    /// Addressing - bto field
    pub bto: Option<Vec<String>>,

    /// Addressing - bcc field
    pub bcc: Option<Vec<String>>,

    /// Audience field
    pub audience: Option<Vec<String>>,

    /// In reply to (for Notes)
    pub in_reply_to: Option<String>,

    /// Conversation/context
    pub conversation: Option<String>,

    /// Tags (hashtags, mentions)
    pub tag: Option<Vec<TagDocument>>,

    /// Media attachments
    pub attachment: Option<Vec<AttachmentDocument>>,

    /// Language code
    pub language: Option<String>,

    /// Content warning/sensitive flag
    pub sensitive: Option<bool>,

    /// Custom properties
    pub additional_properties: Option<Document>,

    /// Local object flag
    pub local: bool,

    /// Visibility level
    pub visibility: VisibilityLevel,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Interaction counts
    pub reply_count: i64,
    pub like_count: i64,
    pub announce_count: i64,
}

/// Tag document for hashtags and mentions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagDocument {
    pub tag_type: String,
    pub name: String,
    pub href: Option<String>,
}

/// Attachment document for media
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentDocument {
    pub attachment_type: String,
    pub url: String,
    pub media_type: Option<String>,
    pub name: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub duration: Option<i32>,
    pub blurhash: Option<String>,
}

/// Visibility levels for objects
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VisibilityLevel {
    #[serde(rename = "public")]
    Public,
    #[serde(rename = "unlisted")]
    Unlisted,
    #[serde(rename = "followers")]
    Followers,
    #[serde(rename = "direct")]
    Direct,
}

/// Activity document in MongoDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// ActivityPub activity ID
    pub activity_id: String,

    /// Activity type
    pub activity_type: ActivityType,

    /// Actor performing the activity
    pub actor: String,

    /// Object of the activity
    pub object: Option<String>,

    /// Target of the activity
    pub target: Option<String>,

    /// Activity name/title
    pub name: Option<String>,

    /// Activity summary
    pub summary: Option<String>,

    /// Published timestamp
    pub published: Option<DateTime<Utc>>,

    /// Updated timestamp
    pub updated: Option<DateTime<Utc>>,

    /// Addressing - to field
    pub to: Option<Vec<String>>,

    /// Addressing - cc field
    pub cc: Option<Vec<String>>,

    /// Addressing - bto field
    pub bto: Option<Vec<String>>,

    /// Addressing - bcc field
    pub bcc: Option<Vec<String>>,

    /// Custom properties
    pub additional_properties: Option<Document>,

    /// Local activity flag
    pub local: bool,

    /// Processing status
    pub status: ActivityStatus,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Processing attempts
    pub attempts: i32,

    /// Last attempt timestamp
    pub last_attempt: Option<DateTime<Utc>>,

    /// Error message if processing failed
    pub error: Option<String>,
}

/// Activity processing status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActivityStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "processing")]
    Processing,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// Key document for PKI system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Key identifier URL
    pub key_id: String,

    /// Actor this key belongs to
    pub actor_id: String,

    /// Key type (user, domain, master, instance)
    pub key_type: KeyType,

    /// Cryptographic algorithm
    pub algorithm: String,

    /// Key size (for RSA)
    pub key_size: Option<u32>,

    /// Public key PEM
    pub public_key_pem: String,

    /// Private key PEM (encrypted)
    pub private_key_pem: Option<String>,

    /// Encryption algorithm for private key
    pub encryption_algorithm: Option<String>,

    /// Key fingerprint
    pub fingerprint: String,

    /// Trust level
    pub trust_level: TrustLevel,

    /// Domain signature (for user keys)
    pub domain_signature: Option<Document>,

    /// Master signature (for domain keys)
    pub master_signature: Option<Document>,

    /// Key usage flags
    pub usage: Vec<String>,

    /// Key status
    pub status: KeyStatus,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,

    /// Rotation policy
    pub rotation_policy: Option<Document>,

    /// Associated domain (for domain keys)
    pub domain: Option<String>,
}

/// Key types in the PKI hierarchy
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KeyType {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "domain")]
    Domain,
    #[serde(rename = "master")]
    Master,
    #[serde(rename = "instance")]
    Instance,
}

/// Key status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KeyStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "revoked")]
    Revoked,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "pending")]
    Pending,
}

/// Domain configuration document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Domain name
    pub domain: String,

    /// Domain display name
    pub name: Option<String>,

    /// Domain description
    pub description: Option<String>,

    /// Domain contact email
    pub contact_email: Option<String>,

    /// Domain rules/terms
    pub rules: Option<Vec<String>>,

    /// Registration mode
    pub registration_mode: RegistrationMode,

    /// Authorized fetch mode
    pub authorized_fetch: bool,

    /// Maximum note length
    pub max_note_length: Option<i32>,

    /// Maximum file size
    pub max_file_size: Option<i64>,

    /// Allowed file types
    pub allowed_file_types: Option<Vec<String>>,

    /// Domain key ID
    pub domain_key_id: Option<String>,

    /// Custom configuration
    pub config: Option<Document>,

    /// Domain status
    pub status: DomainStatus,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Registration modes for domains
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RegistrationMode {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "approval")]
    Approval,
    #[serde(rename = "invite")]
    Invite,
    #[serde(rename = "closed")]
    Closed,
}

/// Domain status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DomainStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "suspended")]
    Suspended,
    #[serde(rename = "maintenance")]
    Maintenance,
}

/// Follow relationship document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowDocument {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,

    /// Actor doing the following
    pub follower: String,

    /// Actor being followed
    pub following: String,

    /// Follow status
    pub status: FollowStatus,

    /// Follow activity ID
    pub activity_id: String,

    /// Accept activity ID (if accepted)
    pub accept_activity_id: Option<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Accept/reject timestamp
    pub responded_at: Option<DateTime<Utc>>,
}

/// Follow relationship status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FollowStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// Database manager for MongoDB operations
pub struct DatabaseManager {
    pub database: Database,
}

impl DatabaseManager {
    /// Create a new database manager
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    /// Initialize database collections and indexes
    pub async fn initialize(&self) -> Result<(), DatabaseError> {
        self.create_indexes().await?;
        Ok(())
    }

    /// Create database indexes for performance
    async fn create_indexes(&self) -> Result<(), DatabaseError> {
        // Actor indexes
        let actors: Collection<ActorDocument> = self.database.collection("actors");
        actors
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "actor_id": 1 })
                    .options(IndexOptions::builder().unique(true).build())
                    .build(),
            )
            .await?;

        actors
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "domain": 1, "preferred_username": 1 })
                    .options(IndexOptions::builder().unique(true).build())
                    .build(),
            )
            .await?;

        // Object indexes
        let objects: Collection<ObjectDocument> = self.database.collection("objects");
        objects
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "object_id": 1 })
                    .options(IndexOptions::builder().unique(true).build())
                    .build(),
            )
            .await?;

        objects
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "attributed_to": 1, "published": -1 })
                    .build(),
            )
            .await?;

        // Activity indexes
        let activities: Collection<ActivityDocument> = self.database.collection("activities");
        activities
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "activity_id": 1 })
                    .options(IndexOptions::builder().unique(true).build())
                    .build(),
            )
            .await?;

        activities
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "actor": 1, "published": -1 })
                    .build(),
            )
            .await?;

        // Key indexes
        let keys: Collection<KeyDocument> = self.database.collection("keys");
        keys.create_index(
            IndexModel::builder()
                .keys(doc! { "key_id": 1 })
                .options(IndexOptions::builder().unique(true).build())
                .build(),
        )
        .await?;

        keys.create_index(IndexModel::builder().keys(doc! { "actor_id": 1 }).build())
            .await?;

        // Domain indexes
        let domains: Collection<DomainDocument> = self.database.collection("domains");
        domains
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "domain": 1 })
                    .options(IndexOptions::builder().unique(true).build())
                    .build(),
            )
            .await?;

        // Follow indexes
        let follows: Collection<FollowDocument> = self.database.collection("follows");
        follows
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "follower": 1, "following": 1 })
                    .options(IndexOptions::builder().unique(true).build())
                    .build(),
            )
            .await?;

        Ok(())
    }

    /// Insert a new actor
    pub async fn insert_actor(&self, actor: ActorDocument) -> Result<ObjectId, DatabaseError> {
        let collection: Collection<ActorDocument> = self.database.collection("actors");
        let result = collection.insert_one(actor).await?;
        Ok(result.inserted_id.as_object_id().unwrap())
    }

    /// Find actor by ID
    pub async fn find_actor_by_id(
        &self,
        actor_id: &str,
    ) -> Result<Option<ActorDocument>, DatabaseError> {
        let collection: Collection<ActorDocument> = self.database.collection("actors");
        let result = collection.find_one(doc! { "actor_id": actor_id }).await?;
        Ok(result)
    }

    /// Find actor by username and domain
    pub async fn find_actor_by_username(
        &self,
        username: &str,
        domain: &str,
    ) -> Result<Option<ActorDocument>, DatabaseError> {
        let collection: Collection<ActorDocument> = self.database.collection("actors");
        let result = collection
            .find_one(doc! { "preferred_username": username, "domain": domain })
            .await?;
        Ok(result)
    }

    /// Update actor
    pub async fn update_actor(
        &self,
        actor_id: &str,
        update: Document,
    ) -> Result<UpdateResult, DatabaseError> {
        let collection: Collection<ActorDocument> = self.database.collection("actors");
        let result = collection
            .update_one(
                doc! { "actor_id": actor_id },
                doc! { "$set": update, "$currentDate": { "updated_at": true } },
            )
            .await?;
        Ok(result)
    }

    /// Insert a new object
    pub async fn insert_object(&self, object: ObjectDocument) -> Result<ObjectId, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let result = collection.insert_one(object).await?;
        Ok(result.inserted_id.as_object_id().unwrap())
    }

    /// Find object by ID
    pub async fn find_object_by_id(
        &self,
        object_id: &str,
    ) -> Result<Option<ObjectDocument>, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let result = collection.find_one(doc! { "object_id": object_id }).await?;
        Ok(result)
    }

    /// Insert a new activity
    pub async fn insert_activity(
        &self,
        activity: ActivityDocument,
    ) -> Result<ObjectId, DatabaseError> {
        let collection: Collection<ActivityDocument> = self.database.collection("activities");
        let result = collection.insert_one(activity).await?;
        Ok(result.inserted_id.as_object_id().unwrap())
    }

    /// Find activity by ID
    pub async fn find_activity_by_id(
        &self,
        activity_id: &str,
    ) -> Result<Option<ActivityDocument>, DatabaseError> {
        let collection: Collection<ActivityDocument> = self.database.collection("activities");
        let result = collection
            .find_one(doc! { "activity_id": activity_id })
            .await?;
        Ok(result)
    }

    /// Insert a new key
    pub async fn insert_key(&self, key: KeyDocument) -> Result<ObjectId, DatabaseError> {
        let collection: Collection<KeyDocument> = self.database.collection("keys");
        let result = collection.insert_one(key).await?;
        Ok(result.inserted_id.as_object_id().unwrap())
    }

    /// Find key by ID
    pub async fn find_key_by_id(&self, key_id: &str) -> Result<Option<KeyDocument>, DatabaseError> {
        let collection: Collection<KeyDocument> = self.database.collection("keys");
        let result = collection.find_one(doc! { "key_id": key_id }).await?;
        Ok(result)
    }

    /// Find keys by actor ID
    pub async fn find_keys_by_actor(
        &self,
        actor_id: &str,
    ) -> Result<Vec<KeyDocument>, DatabaseError> {
        let collection: Collection<KeyDocument> = self.database.collection("keys");
        let mut cursor = collection.find(doc! { "actor_id": actor_id }).await?;
        let mut keys = Vec::new();

        while cursor.advance().await? {
            keys.push(cursor.deserialize_current()?);
        }

        Ok(keys)
    }

    /// Insert a new domain
    pub async fn insert_domain(&self, domain: DomainDocument) -> Result<ObjectId, DatabaseError> {
        let collection: Collection<DomainDocument> = self.database.collection("domains");
        let result = collection.insert_one(domain).await?;
        Ok(result.inserted_id.as_object_id().unwrap())
    }

    /// Find domain by name
    pub async fn find_domain_by_name(
        &self,
        domain_name: &str,
    ) -> Result<Option<DomainDocument>, DatabaseError> {
        let collection: Collection<DomainDocument> = self.database.collection("domains");
        let result = collection.find_one(doc! { "domain": domain_name }).await?;
        Ok(result)
    }

    /// Insert a new follow relationship
    pub async fn insert_follow(&self, follow: FollowDocument) -> Result<ObjectId, DatabaseError> {
        let collection: Collection<FollowDocument> = self.database.collection("follows");
        let result = collection.insert_one(follow).await?;
        Ok(result.inserted_id.as_object_id().unwrap())
    }

    /// Find follow relationship
    pub async fn find_follow(
        &self,
        follower: &str,
        following: &str,
    ) -> Result<Option<FollowDocument>, DatabaseError> {
        let collection: Collection<FollowDocument> = self.database.collection("follows");
        let result = collection
            .find_one(doc! { "follower": follower, "following": following })
            .await?;
        Ok(result)
    }

    /// Update follow status
    pub async fn update_follow_status(
        &self,
        follower: &str,
        following: &str,
        status: FollowStatus,
    ) -> Result<UpdateResult, DatabaseError> {
        let collection: Collection<FollowDocument> = self.database.collection("follows");
        let result = collection
            .update_one(
                doc! { "follower": follower, "following": following },
                doc! {
                    "$set": { "status": mongodb::bson::to_bson(&status)? },
                    "$currentDate": { "responded_at": true }
                },
            )
            .await?;
        Ok(result)
    }

    /// Get actor's outbox (recent objects)
    pub async fn get_actor_outbox(
        &self,
        actor_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ObjectDocument>, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let mut cursor = collection
            .find(doc! { "attributed_to": actor_id })
            .sort(doc! { "published": -1 })
            .limit(limit)
            .skip(offset as u64)
            .await?;

        let mut objects = Vec::new();
        while cursor.advance().await? {
            objects.push(cursor.deserialize_current()?);
        }

        Ok(objects)
    }

    /// Get actor's followers
    pub async fn get_actor_followers(&self, actor_id: &str) -> Result<Vec<String>, DatabaseError> {
        let collection: Collection<FollowDocument> = self.database.collection("follows");
        let mut cursor = collection
            .find(doc! { "following": actor_id, "status": "accepted" })
            .await?;

        let mut followers = Vec::new();
        while cursor.advance().await? {
            let follow: FollowDocument = cursor.deserialize_current()?;
            followers.push(follow.follower);
        }

        Ok(followers)
    }

    /// Get actor's following
    pub async fn get_actor_following(&self, actor_id: &str) -> Result<Vec<String>, DatabaseError> {
        let collection: Collection<FollowDocument> = self.database.collection("follows");
        let mut cursor = collection
            .find(doc! { "follower": actor_id, "status": "accepted" })
            .await?;

        let mut following = Vec::new();
        while cursor.advance().await? {
            let follow: FollowDocument = cursor.deserialize_current()?;
            following.push(follow.following);
        }

        Ok(following)
    }

    /// Update an object
    pub async fn update_object(
        &self,
        object_id: &str,
        update: Document,
    ) -> Result<UpdateResult, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let result = collection
            .update_one(
                doc! { "object_id": object_id },
                doc! { "$set": update, "$currentDate": { "updated_at": true } },
            )
            .await?;
        Ok(result)
    }

    /// Delete an object
    pub async fn delete_object(&self, object_id: &str) -> Result<(), DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        collection
            .delete_one(doc! { "object_id": object_id })
            .await?;
        Ok(())
    }

    /// Update an activity
    pub async fn update_activity(
        &self,
        activity_id: &str,
        update: Document,
    ) -> Result<UpdateResult, DatabaseError> {
        let collection: Collection<ActivityDocument> = self.database.collection("activities");
        let result = collection
            .update_one(
                doc! { "activity_id": activity_id },
                doc! { "$set": update, "$currentDate": { "updated_at": true } },
            )
            .await?;
        Ok(result)
    }

    /// Delete an activity
    pub async fn delete_activity(&self, activity_id: &str) -> Result<(), DatabaseError> {
        let collection: Collection<ActivityDocument> = self.database.collection("activities");
        collection
            .delete_one(doc! { "activity_id": activity_id })
            .await?;
        Ok(())
    }

    /// Find objects by actor with pagination
    pub async fn find_objects_by_actor(
        &self,
        actor_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ObjectDocument>, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let mut cursor = collection
            .find(doc! { "attributed_to": actor_id })
            .sort(doc! { "published": -1 })
            .limit(limit)
            .skip(offset as u64)
            .await?;

        let mut objects = Vec::new();
        while cursor.advance().await? {
            objects.push(cursor.deserialize_current()?);
        }

        Ok(objects)
    }

    /// Find activities by actor with pagination
    pub async fn find_activities_by_actor(
        &self,
        actor_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ActivityDocument>, DatabaseError> {
        let collection: Collection<ActivityDocument> = self.database.collection("activities");
        let mut cursor = collection
            .find(doc! { "actor": actor_id })
            .sort(doc! { "published": -1 })
            .limit(limit)
            .skip(offset as u64)
            .await?;

        let mut activities = Vec::new();
        while cursor.advance().await? {
            activities.push(cursor.deserialize_current()?);
        }

        Ok(activities)
    }

    /// Find activities by type
    pub async fn find_activities_by_type(
        &self,
        activity_type: ActivityType,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ActivityDocument>, DatabaseError> {
        let collection: Collection<ActivityDocument> = self.database.collection("activities");
        let mut cursor = collection
            .find(doc! { "activity_type": mongodb::bson::to_bson(&activity_type)? })
            .sort(doc! { "created_at": -1 })
            .limit(limit)
            .skip(offset as u64)
            .await?;

        let mut activities = Vec::new();
        while cursor.advance().await? {
            activities.push(cursor.deserialize_current()?);
        }

        Ok(activities)
    }

    /// Count objects by actor
    pub async fn count_objects_by_actor(&self, actor_id: &str) -> Result<u64, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let count = collection
            .count_documents(doc! { "attributed_to": actor_id })
            .await?;
        Ok(count)
    }

    /// Update actor counts (followers, following, statuses)
    pub async fn update_actor_counts(
        &self,
        actor_id: &str,
        followers_count: Option<i64>,
        following_count: Option<i64>,
        statuses_count: Option<i64>,
    ) -> Result<UpdateResult, DatabaseError> {
        let mut update_doc = doc! {};

        if let Some(count) = followers_count {
            update_doc.insert("followers_count", count);
        }
        if let Some(count) = following_count {
            update_doc.insert("following_count", count);
        }
        if let Some(count) = statuses_count {
            update_doc.insert("statuses_count", count);
        }

        if !update_doc.is_empty() {
            let system_time: SystemTime = chrono::Utc::now().into();
            update_doc.insert("updated_at", Bson::DateTime(system_time.into()));
            self.update_actor(actor_id, update_doc).await
        } else {
            // Return a minimal successful update result
            let collection: Collection<ActorDocument> = self.database.collection("actors");
            let result = collection
                .update_one(doc! { "actor_id": actor_id }, doc! {})
                .await?;
            Ok(result)
        }
    }

    /// Delete actor and all related data
    pub async fn delete_actor(&self, actor_id: &str) -> Result<(), DatabaseError> {
        // Delete actor
        let actors: Collection<ActorDocument> = self.database.collection("actors");
        actors.delete_one(doc! { "actor_id": actor_id }).await?;

        // Delete actor's objects
        let objects: Collection<ObjectDocument> = self.database.collection("objects");
        objects
            .delete_many(doc! { "attributed_to": actor_id })
            .await?;

        // Delete actor's activities
        let activities: Collection<ActivityDocument> = self.database.collection("activities");
        activities.delete_many(doc! { "actor": actor_id }).await?;

        // Delete actor's keys
        let keys: Collection<KeyDocument> = self.database.collection("keys");
        keys.delete_many(doc! { "actor_id": actor_id }).await?;

        // Delete follow relationships
        let follows: Collection<FollowDocument> = self.database.collection("follows");
        follows
            .delete_many(doc! { "$or": [{"follower": actor_id}, {"following": actor_id}] })
            .await?;

        Ok(())
    }

    /// Get total number of local actors
    pub async fn count_local_actors(&self) -> Result<u64, DatabaseError> {
        let collection: Collection<ActorDocument> = self.database.collection("actors");
        let count = collection.count_documents(doc! { "local": true }).await?;
        Ok(count)
    }

    /// Get total number of local posts
    pub async fn count_local_posts(&self) -> Result<u64, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let count = collection
            .count_documents(doc! {
                "local": true,
                "object_type": { "$in": ["Note", "Article"] }
            })
            .await?;
        Ok(count)
    }

    /// Find objects by content search
    pub async fn search_objects(
        &self,
        query: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ObjectDocument>, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let filter = doc! {
            "$or": [
                { "content": { "$regex": query, "$options": "i" } },
                { "summary": { "$regex": query, "$options": "i" } },
                { "name": { "$regex": query, "$options": "i" } }
            ]
        };

        let cursor = collection
            .find(filter)
            .skip(offset as u64)
            .limit(limit)
            .await?;

        let results: Vec<ObjectDocument> = cursor.try_collect().await?;
        Ok(results)
    }

    /// Get recent public activities for timeline
    pub async fn get_public_timeline(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ObjectDocument>, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let filter = doc! {
            "visibility": "public",
            "object_type": { "$in": ["Note", "Article"] }
        };

        let cursor = collection
            .find(filter)
            .sort(doc! { "published": -1 })
            .skip(offset as u64)
            .limit(limit)
            .await?;

        let results: Vec<ObjectDocument> = cursor.try_collect().await?;
        Ok(results)
    }

    /// Get local timeline (only local posts)
    pub async fn get_local_timeline(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ObjectDocument>, DatabaseError> {
        let collection: Collection<ObjectDocument> = self.database.collection("objects");
        let filter = doc! {
            "local": true,
            "visibility": "public",
            "object_type": { "$in": ["Note", "Article"] }
        };

        let cursor = collection
            .find(filter)
            .sort(doc! { "published": -1 })
            .skip(offset as u64)
            .limit(limit)
            .await?;

        let results: Vec<ObjectDocument> = cursor.try_collect().await?;
        Ok(results)
    }

    /// Update key status
    pub async fn update_key_status(
        &self,
        key_id: &str,
        status: KeyStatus,
    ) -> Result<UpdateResult, DatabaseError> {
        let collection: Collection<KeyDocument> = self.database.collection("keys");
        let status_str = match status {
            KeyStatus::Active => "active",
            KeyStatus::Revoked => "revoked",
            KeyStatus::Expired => "expired",
            KeyStatus::Pending => "pending",
        };
        let result = collection
            .update_one(
                doc! { "key_id": key_id },
                doc! {
                    "$set": { "status": status_str },
                    "$currentDate": { "updated_at": true }
                },
            )
            .await?;
        Ok(result)
    }

    /// Find active keys by actor
    pub async fn find_active_keys_by_actor(
        &self,
        actor_id: &str,
    ) -> Result<Vec<KeyDocument>, DatabaseError> {
        let collection: Collection<KeyDocument> = self.database.collection("keys");
        let filter = doc! {
            "actor_id": actor_id,
            "status": "active"
        };

        let cursor = collection.find(filter).await?;
        let results: Vec<KeyDocument> = cursor.try_collect().await?;
        Ok(results)
    }

    /// Get domain statistics
    pub async fn get_domain_stats(&self, domain: &str) -> Result<(u64, u64, u64), DatabaseError> {
        // Get actor count
        let actors: Collection<ActorDocument> = self.database.collection("actors");
        let actor_count = actors.count_documents(doc! { "domain": domain }).await?;

        // Get post count
        let objects: Collection<ObjectDocument> = self.database.collection("objects");
        let post_count = objects
            .count_documents(doc! {
                "object_type": { "$in": ["Note", "Article"] }
            })
            .await?;

        // Get activity count
        let activities: Collection<ActivityDocument> = self.database.collection("activities");
        let activity_count = activities.count_documents(doc! { "local": true }).await?;

        Ok((actor_count, post_count, activity_count))
    }
}
