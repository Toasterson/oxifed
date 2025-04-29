use mongodb::{
    bson::{doc},
    options::{ClientOptions},
    Client, Collection, Database,
};
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

/// Actor record based on ActivityPub
#[derive(Debug, Serialize, Deserialize)]
pub struct Actor {
    pub id: String,
    pub actor_type: String,
    pub name: Option<String>,
    pub username: String,
    pub domain: String,
    pub inbox_url: String,
    pub outbox_url: String,
    pub following_url: Option<String>,
    pub followers_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Link in a profile
#[derive(Debug, Serialize, Deserialize)]
pub struct Link {
    pub title: String,
    pub url: String,
    pub description: Option<String>,
}

/// Personal profile information
#[derive(Debug, Serialize, Deserialize)]
pub struct Profile {
    pub username: String,
    pub handle: String,
    pub links: Vec<Link>,
}

/// MongoDB connection manager
pub struct MongoDB {
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

    /// Get domains collection
    pub fn domains_collection(&self) -> Collection<Domain> {
        self.db.collection("domains")
    }

    /// Get an Actor collection
    pub fn actors_collection(&self) -> Collection<Actor> {
        self.db.collection("actors")
    }

    /// Get profiles collection
    pub fn profiles_collection(&self) -> Collection<Profile> {
        self.db.collection("profiles")
    }
}
