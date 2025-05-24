//! ActivityPub delivery module
//!
//! This module implements ActivityPub-compliant delivery of activities to followers
//! according to the W3C ActivityPub specification section 7.1.

use crate::db::MongoDB;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use futures::stream::{FuturesUnordered, StreamExt};
use mongodb::bson::doc;
use oxifed::client::{ActivityPubClient, ClientError};
use oxifed::{Activity, Collection, ObjectOrLink};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, warn};
use url::Url;

/// Record for storing follower relationships (for delivery compatibility)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FollowerRecord {
    /// The actor ID who is following
    pub actor_id: String,
    /// When the follow relationship was established
    pub followed_at: DateTime<Utc>,
    /// The actor's inbox URL for delivery
    pub inbox_url: String,
    /// Optional shared inbox URL for optimized delivery
    pub shared_inbox_url: Option<String>,
}

/// Maximum number of concurrent deliveries
const MAX_CONCURRENT_DELIVERIES: usize = 50;

/// Maximum retry attempts for failed deliveries
const MAX_RETRY_ATTEMPTS: usize = 3;

/// Base delay for exponential backoff in milliseconds
const BASE_RETRY_DELAY_MS: u64 = 1000;

/// Maximum number of collection items to process
const MAX_COLLECTION_ITEMS: usize = 1000;

/// Delivery errors
#[derive(Error, Debug)]
pub enum DeliveryError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Client error: {0}")]
    ClientError(#[from] ClientError),

    #[error("URL parse error: {0}")]
    UrlError(#[from] url::ParseError),

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("MongoDB error: {0}")]
    MongoError(#[from] mongodb::error::Error),

    #[error("No recipients found for activity")]
    NoRecipients,

    #[error("Invalid actor ID: {0}")]
    InvalidActorId(String),
}

/// Result type for delivery operations
pub type Result<T> = std::result::Result<T, DeliveryError>;

/// Delivery statistics
#[derive(Debug, Default)]
pub struct DeliveryStats {
    pub total_attempts: usize,
    pub successful_deliveries: usize,
    pub failed_deliveries: usize,
    pub shared_inbox_deliveries: usize,
}

/// Delivery target with inbox information
#[derive(Debug, Clone)]
struct DeliveryTarget {
    actor_id: String,
    inbox_url: Url,
    shared_inbox_url: Option<Url>,
}

/// ActivityPub delivery manager
pub struct DeliveryManager {
    db: Arc<MongoDB>,
    client: ActivityPubClient,
}

impl DeliveryManager {
    /// Create a new delivery manager
    pub fn new(db: Arc<MongoDB>, client: ActivityPubClient) -> Self {
        Self { db, client }
    }

    /// Deliver an activity to all appropriate recipients according to ActivityPub spec
    pub async fn deliver_activity(
        &self,
        activity: &Activity,
        actor_username: &str,
    ) -> Result<DeliveryStats> {
        info!(
            "Starting delivery for activity from actor: {}",
            actor_username
        );

        let mut stats = DeliveryStats::default();

        // Extract recipients from activity addressing
        let recipients = self.extract_recipients(activity, actor_username).await?;

        if recipients.is_empty() {
            warn!("No recipients found for activity from {}", actor_username);
            return Err(DeliveryError::NoRecipients);
        }

        info!("Found {} recipients for delivery", recipients.len());

        // Group targets by shared inbox for optimization (Section 7.1.3)
        let delivery_groups = self.group_by_shared_inbox(recipients);

        // Perform deliveries concurrently with rate limiting
        let mut delivery_futures = FuturesUnordered::new();

        for (inbox_url, targets) in delivery_groups {
            let activity_clone = activity.clone();
            let client = self.client.clone();

            delivery_futures.push(async move {
                Self::deliver_to_inbox(client, inbox_url, &activity_clone, targets).await
            });

            // Limit concurrent deliveries
            if delivery_futures.len() >= MAX_CONCURRENT_DELIVERIES {
                if let Some(result) = delivery_futures.next().await {
                    Self::update_stats(&mut stats, result);
                }
            }
        }

        // Wait for remaining deliveries to complete
        while let Some(result) = delivery_futures.next().await {
            Self::update_stats(&mut stats, result);
        }

        info!(
            "Delivery completed. Success: {}, Failed: {}, Shared inbox: {}",
            stats.successful_deliveries, stats.failed_deliveries, stats.shared_inbox_deliveries
        );

        Ok(stats)
    }

    /// Extract recipients from activity addressing according to ActivityPub spec
    async fn extract_recipients(
        &self,
        activity: &Activity,
        actor_username: &str,
    ) -> Result<Vec<DeliveryTarget>> {
        let mut recipients = HashSet::new();

        // Process each addressing field (Section 7.1)
        self.process_addressing_field(&activity.actor, &mut recipients)
            .await?;
        self.process_addressing_field(&activity.object, &mut recipients)
            .await?;
        self.process_addressing_field(&activity.target, &mut recipients)
            .await?;

        // Process additional properties for to, cc, bcc, bto, audience
        if let Some(to_value) = activity.additional_properties.get("to") {
            self.process_addressing_value(to_value, &mut recipients)
                .await?;
        }
        if let Some(cc_value) = activity.additional_properties.get("cc") {
            self.process_addressing_value(cc_value, &mut recipients)
                .await?;
        }
        if let Some(bcc_value) = activity.additional_properties.get("bcc") {
            self.process_addressing_value(bcc_value, &mut recipients)
                .await?;
        }
        if let Some(bto_value) = activity.additional_properties.get("bto") {
            self.process_addressing_value(bto_value, &mut recipients)
                .await?;
        }
        if let Some(audience_value) = activity.additional_properties.get("audience") {
            self.process_addressing_value(audience_value, &mut recipients)
                .await?;
        }

        // Add followers if explicitly addressed
        let followers_url = format!("https://{}/u/{}/followers", "localhost", actor_username); // TODO: use actual domain
        if recipients.contains(&followers_url) {
            recipients.remove(&followers_url);
            let followers = self.get_followers(actor_username).await?;
            for follower in followers {
                let target = DeliveryTarget {
                    actor_id: follower.actor_id,
                    inbox_url: Url::parse(&follower.inbox_url)?,
                    shared_inbox_url: follower
                        .shared_inbox_url
                        .as_ref()
                        .map(|url| Url::parse(url))
                        .transpose()?,
                };
                recipients.insert(target.inbox_url.to_string());
            }
        }

        // Convert string URLs to DeliveryTargets
        let mut delivery_targets = Vec::new();
        for recipient_url in recipients {
            if let Ok(inbox_url) = Url::parse(&recipient_url) {
                // For direct inbox URLs, we don't have actor info, so create minimal target
                delivery_targets.push(DeliveryTarget {
                    actor_id: recipient_url.clone(),
                    inbox_url,
                    shared_inbox_url: None,
                });
            }
        }

        // Exclude the actor themselves (Section 7.1)
        if let Some(ObjectOrLink::Url(actor_url)) = &activity.actor {
            delivery_targets.retain(|target| target.actor_id != actor_url.to_string());
        }

        Ok(delivery_targets)
    }

    /// Process an ObjectOrLink field for addressing
    async fn process_addressing_field(
        &self,
        field: &Option<ObjectOrLink>,
        recipients: &mut HashSet<String>,
    ) -> Result<()> {
        if let Some(object_or_link) = field {
            match object_or_link {
                ObjectOrLink::Url(url) => {
                    recipients.insert(url.to_string());
                }
                ObjectOrLink::Object(obj) => {
                    if let Some(id) = &obj.id {
                        recipients.insert(id.to_string());
                    }
                }
                ObjectOrLink::Link(link) => {
                    if let Some(href) = &link.href {
                        recipients.insert(href.to_string());
                    }
                }
            }
        }
        Ok(())
    }

    /// Process a JSON value for addressing (handles arrays and single values)
    async fn process_addressing_value(
        &self,
        value: &Value,
        recipients: &mut HashSet<String>,
    ) -> Result<()> {
        match value {
            Value::String(url) => {
                if self.is_collection_url(url).await? {
                    self.expand_collection(url, recipients).await?;
                } else {
                    recipients.insert(url.clone());
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    if let Value::String(url) = item {
                        if self.is_collection_url(url).await? {
                            self.expand_collection(url, recipients).await?;
                        } else {
                            recipients.insert(url.clone());
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Check if a URL points to a collection
    async fn is_collection_url(&self, url: &str) -> Result<bool> {
        // Check common collection patterns
        Ok(url.contains("/followers") || url.contains("/following") || url.contains("/audience"))
    }

    /// Expand a collection URL to individual recipients
    async fn expand_collection(
        &self,
        collection_url: &str,
        recipients: &mut HashSet<String>,
    ) -> Result<()> {
        debug!("Expanding collection: {}", collection_url);

        if collection_url.contains("/followers") {
            // Extract username from URL and get followers from database
            if let Some(username) = self.extract_username_from_url(collection_url) {
                let followers = self.get_followers(&username).await?;
                for follower in followers.into_iter().take(MAX_COLLECTION_ITEMS) {
                    recipients.insert(follower.inbox_url);
                }
            }
        } else {
            // For external collections, fetch via HTTP
            if let Ok(url) = Url::parse(collection_url) {
                match self.client.fetch_collection(&url).await {
                    Ok(collection) => {
                        self.process_collection_items(&collection, recipients)
                            .await?;
                    }
                    Err(e) => {
                        warn!("Failed to fetch collection {}: {}", collection_url, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Process items in a collection
    async fn process_collection_items(
        &self,
        collection: &Collection,
        recipients: &mut HashSet<String>,
    ) -> Result<()> {
        for item in collection.items.iter().take(MAX_COLLECTION_ITEMS) {
            match item {
                ObjectOrLink::Url(url) => {
                    // Fetch actor and get their inbox
                    if let Ok(actor) = self.client.fetch_actor(&url).await {
                        if let Some(Value::String(inbox)) = actor.additional_properties.get("inbox")
                        {
                            recipients.insert(inbox.clone());
                        }
                    }
                }
                ObjectOrLink::Object(obj) => {
                    if let Some(Value::String(inbox)) = obj.additional_properties.get("inbox") {
                        recipients.insert(inbox.clone());
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Extract username from a URL
    fn extract_username_from_url(&self, url: &str) -> Option<String> {
        if let Ok(parsed_url) = Url::parse(url) {
            let path = parsed_url.path();
            if let Ok(re) = regex::Regex::new(r"/u/([^/]+)/") {
                if let Some(captures) = re.captures(path) {
                    return captures.get(1).map(|m| m.as_str().to_string());
                }
            }
        }
        None
    }

    /// Get followers from database
    async fn get_followers(&self, username: &str) -> Result<Vec<FollowerRecord>> {
        let actor_id = format!("https://{}/users/{}", "example.com", username); // TODO: get domain from config
        let follower_ids = self.db.manager().get_actor_followers(&actor_id).await
            .map_err(|e| DeliveryError::DatabaseError(e.to_string()))?;
        
        // Convert follower IDs to FollowerRecord format for compatibility
        let followers = follower_ids.into_iter().map(|actor_id| FollowerRecord {
            actor_id: actor_id.clone(),
            followed_at: chrono::Utc::now(), // TODO: get actual timestamp from database
            inbox_url: format!("{}/inbox", actor_id),
            shared_inbox_url: None,
        }).collect();
        
        Ok(followers)
    }

    /// Group delivery targets by shared inbox for optimization
    fn group_by_shared_inbox(
        &self,
        targets: Vec<DeliveryTarget>,
    ) -> HashMap<Url, Vec<DeliveryTarget>> {
        let mut groups: HashMap<Url, Vec<DeliveryTarget>> = HashMap::new();

        for target in targets {
            let inbox_url = if let Some(shared_inbox) = &target.shared_inbox_url {
                shared_inbox.clone()
            } else {
                target.inbox_url.clone()
            };

            groups.entry(inbox_url).or_default().push(target);
        }

        groups
    }

    /// Deliver activity to a specific inbox with retry logic
    async fn deliver_to_inbox(
        client: ActivityPubClient,
        inbox_url: Url,
        activity: &Activity,
        targets: Vec<DeliveryTarget>,
    ) -> (usize, usize, bool) {
        let is_shared_inbox = targets.len() > 1;
        let mut attempts = 0;

        loop {
            attempts += 1;

            match client.send_to_inbox(&inbox_url, activity).await {
                Ok(_) => {
                    debug!("Successfully delivered to {}", inbox_url);
                    return (targets.len(), 0, is_shared_inbox);
                }
                Err(e) => {
                    error!(
                        "Delivery attempt {} failed for {}: {}",
                        attempts, inbox_url, e
                    );

                    if attempts >= MAX_RETRY_ATTEMPTS {
                        error!("Max retry attempts reached for {}", inbox_url);
                        return (0, targets.len(), false);
                    }

                    // Exponential backoff
                    let delay = Duration::from_millis(
                        BASE_RETRY_DELAY_MS * (2_u64.pow(attempts as u32 - 1)),
                    );
                    sleep(delay).await;
                }
            }
        }
    }

    /// Update delivery statistics
    fn update_stats(stats: &mut DeliveryStats, result: (usize, usize, bool)) {
        let (successful, failed, is_shared) = result;
        stats.total_attempts += successful + failed;
        stats.successful_deliveries += successful;
        stats.failed_deliveries += failed;
        if is_shared && successful > 0 {
            stats.shared_inbox_deliveries += 1;
        }
    }
}
