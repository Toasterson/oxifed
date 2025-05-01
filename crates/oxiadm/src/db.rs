use miette::{Context, IntoDiagnostic, Result};
use mongodb::{
    bson::{doc},
    options::ClientOptions,
    Client, Collection,
};
use oxifed::webfinger::JrdResource;
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

/// MongoDB client for profile operations
pub struct MongoClient {
    client: Client,
    db_name: String,
}

impl MongoClient {
    /// Create a new MongoDB client
    pub async fn new(connection_string: &str, db_name: &str) -> Result<Self> {
        let client_options = ClientOptions::parse(connection_string)
            .await
            .into_diagnostic()
            .wrap_err("Failed to parse MongoDB connection string")?;

        let client = Client::with_options(client_options)
            .into_diagnostic()
            .wrap_err("Failed to create MongoDB client")?;

        // Ping the database to ensure connectivity
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .into_diagnostic()
            .wrap_err("Failed to connect to MongoDB")?;

        Ok(Self {
            client,
            db_name: db_name.to_string(),
        })
    }

    /// Get profiles collection
    pub fn profiles_collection(&self) -> Collection<JrdResource> {
        self.client.database(&self.db_name).collection("profiles")
    }

    /// Create a new profile in the database
    pub async fn create_profile(&self, resource: JrdResource) -> Result<()> {
        let profiles = self.profiles_collection();

        // Check if a profile with the same name already exists
        let filter = doc! { "subject": &resource.subject };
        let existing = profiles.find_one(filter.clone()).await
            .into_diagnostic()
            .wrap_err("Failed to check for existing profile")?;

        if existing.is_some() {
            return Err(miette::miette!("Profile with subject '{}' already exists", 
                resource.subject.as_ref().unwrap_or(&String::from("<no subject>"))));
        }

        // Insert the new profile
        profiles
            .insert_one(resource)
            .await
            .into_diagnostic()
            .wrap_err("Failed to insert profile into MongoDB")?;

        Ok(())
    }

    /// Get a profile by name
    pub async fn get_profile(&self, subject: &str) -> Result<JrdResource> {
        let profiles = self.profiles_collection();
        let filter = doc! { "subject": subject };

        let profile = profiles
            .find_one(filter)
            .await
            .into_diagnostic()
            .wrap_err("Failed to retrieve profile from MongoDB")?;

        profile.ok_or_else(|| miette::miette!("Profile with subject '{}' not found", subject))
    }

    /// Update an existing profile
    pub async fn update_profile(&self, subject: &str, resource: JrdResource) -> Result<()> {
        let profiles = self.profiles_collection();
        let filter = doc! { "subject": subject };

        // Check if profile exists
        let existing = profiles
            .find_one(filter.clone())
            .await
            .into_diagnostic()
            .wrap_err("Failed to check for existing profile")?;

        if existing.is_none() {
            return Err(miette::miette!("Profile with subject '{}' not found", subject));
        }

        // Update the profile
        profiles
            .replace_one(filter, resource)
            .await
            .into_diagnostic()
            .wrap_err("Failed to update profile in MongoDB")?;

        Ok(())
    }
}
