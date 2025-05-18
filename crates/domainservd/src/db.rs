use mongodb::{Client, Collection, Database, bson::doc, options::ClientOptions};
use oxifed::webfinger::JrdResource;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// MongoDB-related errors
#[derive(Error, Debug)]
pub enum DbError {
    /// MongoDB error
    #[error("MongoDB error: {0}")]
    MongoError(#[from] mongodb::error::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] mongodb::bson::ser::Error),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] mongodb::bson::de::Error),
}

/// Domain record for storing domain configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct Domain {
    pub domain: String,
    pub enabled: bool,
}

/// MongoDB connection manager
pub struct MongoDB {
    #[allow(dead_code)]
    client: Client,
    db: Database,
}

impl MongoDB {
    /// Create a new MongoDB connection
    pub async fn new(connection_string: &str, db_name: &str) -> Result<Self, DbError> {
        let client_options = ClientOptions::parse(connection_string).await?;
        let client = Client::with_options(client_options)?;
        let db = client.database(db_name);

        Ok(Self { client, db })
    }

    /// Initialize collections
    pub async fn init_collections(&self) -> Result<(), DbError> {
        tracing::info!("Initializing MongoDB collections");

        // Check if collections exist, create them if they don't
        let collection_names = self.db.list_collection_names().await?;

        if !collection_names.contains(&"domains".to_string()) {
            tracing::info!("Creating 'domains' collection");
            self.db.create_collection("domains").await?;
        }

        if !collection_names.contains(&"actors".to_string()) {
            tracing::info!("Creating 'actors' collection");
            self.db.create_collection("actors").await?;
        }

        if !collection_names.contains(&"profiles".to_string()) {
            tracing::info!("Creating 'profiles' collection");
            self.db.create_collection("profiles").await?;
        }

        tracing::info!("MongoDB collections initialized successfully");
        Ok(())
    }

    pub fn domains_collection(&self) -> Collection<Domain> {
        self.db.collection("domains")
    }

    pub fn actors_collection(&self) -> Collection<oxifed::Actor> {
        self.db.collection("actors")
    }

    pub fn outbox_collection(&self, username: &str) -> Collection<oxifed::Object> {
        self.db.collection(&format!("{}.outbox", username))
    }

    pub fn activities_collection(&self, username: &str) -> Collection<oxifed::Activity> {
        self.db.collection(&format!("{}.activities", username))
    }

    /// Get profiles collection
    pub fn webfinger_profiles_collection(&self) -> Collection<JrdResource> {
        self.db.collection("webfinger_profiles")
    }
}
