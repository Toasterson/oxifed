//! Database module for domainservd
//!
//! Provides a clean interface to the comprehensive database implementation.

use mongodb::Database;
use oxifed::database::{ActorDocument, DatabaseError, DatabaseManager, ObjectDocument};
use oxifed::webfinger::JrdResource;
use std::sync::Arc;
use thiserror::Error;

/// Database errors for domainservd
#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database operation failed: {0}")]
    DatabaseError(#[from] DatabaseError),
}

/// Database connection manager for domainservd
pub struct MongoDB {
    manager: Arc<DatabaseManager>,
    database: Database,
}

impl MongoDB {
    /// Create a new MongoDB connection
    pub async fn new(connection_string: &str, db_name: &str) -> Result<Self, DbError> {
        use mongodb::{Client, options::ClientOptions};

        let client_options = ClientOptions::parse(connection_string)
            .await
            .map_err(|e| DbError::DatabaseError(DatabaseError::MongoError(e)))?;
        let client = Client::with_options(client_options)
            .map_err(|e| DbError::DatabaseError(DatabaseError::MongoError(e)))?;
        let database = client.database(db_name);

        let manager = Arc::new(DatabaseManager::new(database.clone()));

        Ok(Self { manager, database })
    }

    /// Get the database manager
    pub fn manager(&self) -> &DatabaseManager {
        &self.manager
    }

    /// Get the raw database instance
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Initialize collections and indexes
    pub async fn init_collections(&self) -> Result<(), DbError> {
        self.manager.initialize().await?;
        Ok(())
    }

    /// Find actor by username and domain
    pub async fn find_actor_by_username(
        &self,
        username: &str,
        domain: &str,
    ) -> Result<Option<ActorDocument>, DbError> {
        self.manager
            .find_actor_by_username(username, domain)
            .await
            .map_err(Into::into)
    }

    /// Find actor by ID
    pub async fn find_actor_by_id(&self, actor_id: &str) -> Result<Option<ActorDocument>, DbError> {
        self.manager
            .find_actor_by_id(actor_id)
            .await
            .map_err(Into::into)
    }

    /// Insert new actor
    pub async fn insert_actor(
        &self,
        actor: ActorDocument,
    ) -> Result<mongodb::bson::oid::ObjectId, DbError> {
        self.manager.insert_actor(actor).await.map_err(Into::into)
    }

    /// Get actor's followers
    pub async fn get_actor_followers(&self, actor_id: &str) -> Result<Vec<String>, DbError> {
        self.manager
            .get_actor_followers(actor_id)
            .await
            .map_err(Into::into)
    }

    /// Get actor's following
    pub async fn get_actor_following(&self, actor_id: &str) -> Result<Vec<String>, DbError> {
        self.manager
            .get_actor_following(actor_id)
            .await
            .map_err(Into::into)
    }

    /// Get actor's outbox
    pub async fn get_actor_outbox(
        &self,
        actor_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ObjectDocument>, DbError> {
        self.manager
            .find_objects_by_actor(actor_id, limit, offset)
            .await
            .map_err(Into::into)
    }

    /// Get WebFinger profiles collection (for specific domainservd functionality)
    pub fn webfinger_profiles_collection(&self) -> mongodb::Collection<JrdResource> {
        self.database.collection("webfinger_profiles")
    }

    /// Get actor's activities (for legacy compatibility)
    pub async fn get_actor_activities(
        &self,
        actor_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<oxifed::database::ActivityDocument>, DbError> {
        self.manager
            .find_activities_by_actor(actor_id, limit, offset)
            .await
            .map_err(Into::into)
    }

    /// Count objects for an actor
    pub async fn count_actor_objects(&self, actor_id: &str) -> Result<u64, DbError> {
        self.manager
            .count_objects_by_actor(actor_id)
            .await
            .map_err(Into::into)
    }

    /// Update actor statistics
    pub async fn update_actor_stats(
        &self,
        actor_id: &str,
        followers_count: Option<i64>,
        following_count: Option<i64>,
        statuses_count: Option<i64>,
    ) -> Result<(), DbError> {
        self.manager
            .update_actor_counts(actor_id, followers_count, following_count, statuses_count)
            .await?;
        Ok(())
    }
}
