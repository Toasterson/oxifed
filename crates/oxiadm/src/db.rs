use miette::{Context, IntoDiagnostic, Result};
use mongodb::{
    bson::{doc, Document},
    options::{ClientOptions, FindOptions},
    Client, Collection, Cursor,
};
use oxifed::webfinger::JrdResource;
use oxifed::{Object, Activity, ActivityPubEntity, ObjectType, ActivityType};
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;
use url::Url;

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

    /// JSON error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// URL parsing error
    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),

    /// Entity not found
    #[error("Entity not found: {0}")]
    NotFound(String),

    /// Invalid ID format
    #[error("Invalid ID format: {0}")]
    InvalidId(String),
}

impl From<DbError> for miette::Error {
    fn from(err: DbError) -> Self {
        miette::Error::msg(err.to_string())
    }
}

/// MongoDB client for operations
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

    /* ----- Profile WebFinger operations ----- */

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

    /* ----- ActivityPub Object operations ----- */

    /// Get objects collection
    pub fn objects_collection(&self) -> Collection<Document> {
        self.client.database(&self.db_name).collection("objects")
    }

    /// Create a new object in the database
    pub async fn create_object(&self, object: &Object) -> std::result::Result<String, DbError> {
        let objects = self.objects_collection();
        
        // Convert to document
        let doc = mongodb::bson::to_document(&object)?;
        
        // Insert the object
        let result = objects.insert_one(doc, None).await?;
        
        // Return the ID
        match result.inserted_id.as_object_id() {
            Some(id) => Ok(id.to_hex()),
            None => Ok(result.inserted_id.to_string()),
        }
    }
    
    /// Get an object by ID
    pub async fn get_object(&self, id: &str) -> std::result::Result<Object, DbError> {
        let objects = self.objects_collection();
        
        // Determine if it's a URL or an ObjectId
        let filter = if id.starts_with("http") {
            doc! { "id": id }
        } else {
            match mongodb::bson::oid::ObjectId::parse_str(id) {
                Ok(oid) => doc! { "_id": oid },
                Err(_) => doc! { "id": id },
            }
        };
        
        let doc = objects.find_one(filter, None).await?
            .ok_or_else(|| DbError::NotFound(id.to_string()))?;
            
        // Convert to Object
        let object: Object = mongodb::bson::from_document(doc)?;
        
        Ok(object)
    }
    
    /// Update an existing object
    pub async fn update_object(&self, id: &str, object: &Object) -> std::result::Result<(), DbError> {
        let objects = self.objects_collection();
        
        // Determine if it's a URL or an ObjectId
        let filter = if id.starts_with("http") {
            doc! { "id": id }
        } else {
            match mongodb::bson::oid::ObjectId::parse_str(id) {
                Ok(oid) => doc! { "_id": oid },
                Err(_) => doc! { "id": id },
            }
        };
        
        // Check if object exists
        let existing = objects.find_one(filter.clone(), None).await?;
        
        if existing.is_none() {
            return Err(DbError::NotFound(id.to_string()));
        }
        
        // Convert to document
        let doc = mongodb::bson::to_document(&object)?;
        
        // Update the object
        objects.replace_one(filter, doc, None).await?;
        
        Ok(())
    }
    
    /// Delete an object
    pub async fn delete_object(&self, id: &str) -> std::result::Result<(), DbError> {
        let objects = self.objects_collection();
        
        // Determine if it's a URL or an ObjectId
        let filter = if id.starts_with("http") {
            doc! { "id": id }
        } else {
            match mongodb::bson::oid::ObjectId::parse_str(id) {
                Ok(oid) => doc! { "_id": oid },
                Err(_) => doc! { "id": id },
            }
        };
        
        // Check if object exists
        let existing = objects.find_one(filter.clone(), None).await?;
        
        if existing.is_none() {
            return Err(DbError::NotFound(id.to_string()));
        }
        
        // Delete the object
        objects.delete_one(filter, None).await?;
        
        Ok(())
    }
    
    /// List objects with filter
    pub async fn list_objects(&self, 
                             type_filter: Option<ObjectType>,
                             name_filter: Option<&str>,
                             content_filter: Option<&str>,
                             author_filter: Option<&str>, 
                             limit: usize) 
        -> std::result::Result<Vec<Object>, DbError> {
        let objects = self.objects_collection();
        let mut filter = Document::new();
        
        // Apply type filter
        if let Some(obj_type) = type_filter {
            filter.insert("object_type", mongodb::bson::to_bson(&obj_type)?);
        }
        
        // Apply name filter if provided
        if let Some(name) = name_filter {
            filter.insert("name", doc! { "$regex": name, "$options": "i" });
        }
        
        // Apply content filter if provided
        if let Some(content) = content_filter {
            filter.insert("content", doc! { "$regex": content, "$options": "i" });
        }
        
        // Apply author filter if provided
        if let Some(author) = author_filter {
            // Author could be in attributed_to field
            if author.starts_with("http") {
                filter.insert("attributed_to", author);
            } else {
                // Try to match with a regex on the URL
                filter.insert("attributed_to", doc! { 
                    "$regex": format!(".*{}.*", author), 
                    "$options": "i" 
                });
            }
        }
        
        // Set options with limit
        let options = FindOptions::builder()
            .limit(Some(limit as i64))
            .build();
            
        // Execute query
        let cursor = objects.find(filter, options).await?;
        
        // Collect results
        let docs: Vec<Document> = cursor.collect().await?;
        let mut result = Vec::with_capacity(docs.len());
        
        // Convert documents to objects
        for doc in docs {
            let object: Object = mongodb::bson::from_document(doc)?;
            result.push(object);
        }
        
        Ok(result)
    }
    
    /* ----- ActivityPub Activity operations ----- */
    
    /// Get activities collection
    pub fn activities_collection(&self) -> Collection<Document> {
        self.client.database(&self.db_name).collection("activities")
    }
    
    /// Create a new activity in the database
    pub async fn create_activity(&self, activity: &Activity) -> std::result::Result<String, DbError> {
        let activities = self.activities_collection();
        
        // Convert to document
        let doc = mongodb::bson::to_document(&activity)?;
        
        // Insert the activity
        let result = activities.insert_one(doc, None).await?;
        
        // Return the ID
        match result.inserted_id.as_object_id() {
            Some(id) => Ok(id.to_hex()),
            None => Ok(result.inserted_id.to_string()),
        }
    }
    
    /// Get an activity by ID
    pub async fn get_activity(&self, id: &str) -> std::result::Result<Activity, DbError> {
        let activities = self.activities_collection();
        
        // Determine if it's a URL or an ObjectId
        let filter = if id.starts_with("http") {
            doc! { "id": id }
        } else {
            match mongodb::bson::oid::ObjectId::parse_str(id) {
                Ok(oid) => doc! { "_id": oid },
                Err(_) => doc! { "id": id },
            }
        };
        
        let doc = activities.find_one(filter, None).await?
            .ok_or_else(|| DbError::NotFound(id.to_string()))?;
            
        // Convert to Activity
        let activity: Activity = mongodb::bson::from_document(doc)?;
        
        Ok(activity)
    }
    
    /// Delete an activity
    pub async fn delete_activity(&self, id: &str) -> std::result::Result<(), DbError> {
        let activities = self.activities_collection();
        
        // Determine if it's a URL or an ObjectId
        let filter = if id.starts_with("http") {
            doc! { "id": id }
        } else {
            match mongodb::bson::oid::ObjectId::parse_str(id) {
                Ok(oid) => doc! { "_id": oid },
                Err(_) => doc! { "id": id },
            }
        };
        
        // Check if activity exists
        let existing = activities.find_one(filter.clone(), None).await?;
        
        if existing.is_none() {
            return Err(DbError::NotFound(id.to_string()));
        }
        
        // Delete the activity
        activities.delete_one(filter, None).await?;
        
        Ok(())
    }
    
    /// List activities with filter
    pub async fn list_activities(&self, 
                               actor_filter: Option<&str>,
                               type_filter: Option<ActivityType>,
                               limit: usize) 
        -> std::result::Result<Vec<Activity>, DbError> {
        let activities = self.activities_collection();
        let mut filter = Document::new();
        
        // Apply actor filter if provided
        if let Some(actor) = actor_filter {
            if actor.starts_with("http") {
                filter.insert("actor", actor);
            } else {
                // Try to match with a regex on the URL
                filter.insert("actor", doc! { 
                    "$regex": format!(".*{}.*", actor), 
                    "$options": "i" 
                });
            }
        }
        
        // Apply type filter
        if let Some(act_type) = type_filter {
            filter.insert("activity_type", mongodb::bson::to_bson(&act_type)?);
        }
        
        // Set options with limit
        let options = FindOptions::builder()
            .limit(Some(limit as i64))
            .build();
            
        // Execute query
        let cursor = activities.find(filter, options).await?;
        
        // Collect results
        let docs: Vec<Document> = cursor.collect().await?;
        let mut result = Vec::with_capacity(docs.len());
        
        // Convert documents to activities
        for doc in docs {
            let activity: Activity = mongodb::bson::from_document(doc)?;
            result.push(activity);
        }
        
        Ok(result)
    }
}
