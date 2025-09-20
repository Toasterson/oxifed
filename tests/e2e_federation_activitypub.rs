//! End-to-end ActivityPub workflow tests for oxifed
//!
//! This test suite validates comprehensive ActivityPub workflows including:
//! - Follow/Unfollow with Accept/Reject responses
//! - Like/Unlike activities
//! - Announce (boost/repost) functionality
//! - Activity ordering and federation

use std::collections::HashMap;
use std::env;
use std::time::Duration;

use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use tracing_subscriber;
use uuid::Uuid;

// Test configuration
struct TestConfig {
    solarm_url: String,
    space_url: String,
    aopc_url: String,
    mongodb_uri: String,
    amqp_uri: String,
}

impl TestConfig {
    fn from_env() -> Self {
        TestConfig {
            solarm_url: env::var("SOLARM_URL")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            space_url: env::var("SPACE_URL")
                .unwrap_or_else(|_| "http://localhost:8082".to_string()),
            aopc_url: env::var("AOPC_URL").unwrap_or_else(|_| "http://localhost:8083".to_string()),
            mongodb_uri: env::var("MONGODB_URI").unwrap_or_else(|_| {
                "mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin".to_string()
            }),
            amqp_uri: env::var("AMQP_URI")
                .unwrap_or_else(|_| "amqp://admin:testpassword@localhost:5672".to_string()),
        }
    }
}

// ActivityPub Activity types
#[derive(Debug, Serialize, Deserialize)]
struct Follow {
    #[serde(rename = "@context")]
    context: Value,
    id: String,
    #[serde(rename = "type")]
    type_: String,
    actor: String,
    object: String,
    published: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Accept {
    #[serde(rename = "@context")]
    context: Value,
    id: String,
    #[serde(rename = "type")]
    type_: String,
    actor: String,
    object: Value,
    published: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Reject {
    #[serde(rename = "@context")]
    context: Value,
    id: String,
    #[serde(rename = "type")]
    type_: String,
    actor: String,
    object: Value,
    published: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Like {
    #[serde(rename = "@context")]
    context: Value,
    id: String,
    #[serde(rename = "type")]
    type_: String,
    actor: String,
    object: String,
    published: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Announce {
    #[serde(rename = "@context")]
    context: Value,
    id: String,
    #[serde(rename = "type")]
    type_: String,
    actor: String,
    object: String,
    published: String,
    to: Vec<String>,
    cc: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Undo {
    #[serde(rename = "@context")]
    context: Value,
    id: String,
    #[serde(rename = "type")]
    type_: String,
    actor: String,
    object: Value,
    published: String,
}

// Test helper for ActivityPub workflows
struct ActivityPubTestHelper {
    client: Client,
    config: TestConfig,
}

impl ActivityPubTestHelper {
    fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        ActivityPubTestHelper {
            client,
            config: TestConfig::from_env(),
        }
    }

    async fn wait_for_services(&self) -> Result<(), String> {
        let services = vec![
            (&self.config.solarm_url, "social.solarm.org"),
            (&self.config.space_url, "solarm.space"),
            (&self.config.aopc_url, "social.aopc.cloud"),
        ];

        for (url, name) in services {
            info!("Waiting for {} to be healthy...", name);
            let health_url = format!("{}/health", url);

            for i in 0..30 {
                match self.client.get(&health_url).send().await {
                    Ok(response) if response.status().is_success() => {
                        info!("✓ {} is healthy", name);
                        break;
                    }
                    _ => {
                        if i == 29 {
                            return Err(format!("{} failed to become healthy", name));
                        }
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }

        Ok(())
    }

    async fn create_test_actor(
        &self,
        base_url: &str,
        domain: &str,
        username: &str,
    ) -> Result<String, String> {
        let actor_endpoint = format!("{}/api/v1/actors", base_url);
        let actor_id = format!("{}/users/{}", base_url, username);

        let actor_data = json!({
            "username": username,
            "display_name": format!("{} Test User", username),
            "bio": format!("Test user for ActivityPub workflows on {}", domain),
            "domain": domain,
            "manually_approves_followers": username == "selective"  // Make one actor selective
        });

        let response = self
            .client
            .post(&actor_endpoint)
            .json(&actor_data)
            .send()
            .await
            .map_err(|e| format!("Failed to create actor: {}", e))?;

        if response.status().is_success() || response.status() == StatusCode::CONFLICT {
            info!("Actor {} created/exists on {}", username, domain);
            Ok(actor_id)
        } else {
            Err(format!(
                "Failed to create actor: Status {}",
                response.status()
            ))
        }
    }

    async fn send_follow(
        &self,
        from_url: &str,
        from_actor: &str,
        to_actor_url: &str,
    ) -> Result<String, String> {
        let follow_id = format!("{}/activities/follow-{}", from_url, Uuid::new_v4());
        let actor_url = format!("{}/users/{}", from_url, from_actor);
        let outbox_url = format!("{}/outbox", actor_url);

        let follow_activity = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": follow_id.clone(),
            "type": "Follow",
            "actor": actor_url,
            "object": to_actor_url,
            "published": Utc::now().to_rfc3339()
        });

        let response = self
            .client
            .post(&outbox_url)
            .header("Content-Type", "application/activity+json")
            .json(&follow_activity)
            .send()
            .await
            .map_err(|e| format!("Failed to send follow: {}", e))?;

        if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
            info!("{} sent follow request to {}", from_actor, to_actor_url);
            Ok(follow_id)
        } else {
            Err(format!("Failed to send follow: {}", response.status()))
        }
    }

    async fn accept_follow(
        &self,
        actor_url: &str,
        follow_activity: &Value,
    ) -> Result<String, String> {
        let accept_id = format!("{}/activities/accept-{}", actor_url, Uuid::new_v4());
        let inbox_url = format!("{}/inbox", actor_url);

        let accept_activity = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": accept_id.clone(),
            "type": "Accept",
            "actor": actor_url,
            "object": follow_activity,
            "published": Utc::now().to_rfc3339()
        });

        let response = self
            .client
            .post(&inbox_url)
            .header("Content-Type", "application/activity+json")
            .json(&accept_activity)
            .send()
            .await
            .map_err(|e| format!("Failed to send accept: {}", e))?;

        if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
            info!("Follow accepted by {}", actor_url);
            Ok(accept_id)
        } else {
            Err(format!("Failed to accept follow: {}", response.status()))
        }
    }

    async fn reject_follow(
        &self,
        actor_url: &str,
        follow_activity: &Value,
    ) -> Result<String, String> {
        let reject_id = format!("{}/activities/reject-{}", actor_url, Uuid::new_v4());
        let inbox_url = format!("{}/inbox", actor_url);

        let reject_activity = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": reject_id.clone(),
            "type": "Reject",
            "actor": actor_url,
            "object": follow_activity,
            "published": Utc::now().to_rfc3339()
        });

        let response = self
            .client
            .post(&inbox_url)
            .header("Content-Type", "application/activity+json")
            .json(&reject_activity)
            .send()
            .await
            .map_err(|e| format!("Failed to send reject: {}", e))?;

        if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
            info!("Follow rejected by {}", actor_url);
            Ok(reject_id)
        } else {
            Err(format!("Failed to reject follow: {}", response.status()))
        }
    }

    async fn send_like(
        &self,
        from_url: &str,
        from_actor: &str,
        object_url: &str,
    ) -> Result<String, String> {
        let like_id = format!("{}/activities/like-{}", from_url, Uuid::new_v4());
        let actor_url = format!("{}/users/{}", from_url, from_actor);
        let outbox_url = format!("{}/outbox", actor_url);

        let like_activity = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": like_id.clone(),
            "type": "Like",
            "actor": actor_url,
            "object": object_url,
            "published": Utc::now().to_rfc3339()
        });

        let response = self
            .client
            .post(&outbox_url)
            .header("Content-Type", "application/activity+json")
            .json(&like_activity)
            .send()
            .await
            .map_err(|e| format!("Failed to send like: {}", e))?;

        if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
            info!("{} liked {}", from_actor, object_url);
            Ok(like_id)
        } else {
            Err(format!("Failed to send like: {}", response.status()))
        }
    }

    async fn send_announce(
        &self,
        from_url: &str,
        from_actor: &str,
        object_url: &str,
        to: Vec<String>,
    ) -> Result<String, String> {
        let announce_id = format!("{}/activities/announce-{}", from_url, Uuid::new_v4());
        let actor_url = format!("{}/users/{}", from_url, from_actor);
        let outbox_url = format!("{}/outbox", actor_url);

        let announce_activity = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": announce_id.clone(),
            "type": "Announce",
            "actor": actor_url,
            "object": object_url,
            "to": to,
            "cc": ["https://www.w3.org/ns/activitystreams#Public"],
            "published": Utc::now().to_rfc3339()
        });

        let response = self
            .client
            .post(&outbox_url)
            .header("Content-Type", "application/activity+json")
            .json(&announce_activity)
            .send()
            .await
            .map_err(|e| format!("Failed to send announce: {}", e))?;

        if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
            info!("{} announced {}", from_actor, object_url);
            Ok(announce_id)
        } else {
            Err(format!("Failed to send announce: {}", response.status()))
        }
    }

    async fn send_undo(
        &self,
        from_url: &str,
        from_actor: &str,
        activity_to_undo: &Value,
    ) -> Result<String, String> {
        let undo_id = format!("{}/activities/undo-{}", from_url, Uuid::new_v4());
        let actor_url = format!("{}/users/{}", from_url, from_actor);
        let outbox_url = format!("{}/outbox", actor_url);

        let undo_activity = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "id": undo_id.clone(),
            "type": "Undo",
            "actor": actor_url,
            "object": activity_to_undo,
            "published": Utc::now().to_rfc3339()
        });

        let response = self
            .client
            .post(&outbox_url)
            .header("Content-Type", "application/activity+json")
            .json(&undo_activity)
            .send()
            .await
            .map_err(|e| format!("Failed to send undo: {}", e))?;

        if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
            info!("{} sent undo for activity", from_actor);
            Ok(undo_id)
        } else {
            Err(format!("Failed to send undo: {}", response.status()))
        }
    }

    async fn create_note(
        &self,
        from_url: &str,
        from_actor: &str,
        content: &str,
    ) -> Result<String, String> {
        let note_id = format!("{}/users/{}/notes/{}", from_url, from_actor, Uuid::new_v4());
        let actor_url = format!("{}/users/{}", from_url, from_actor);
        let outbox_url = format!("{}/outbox", actor_url);

        let create_activity = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Create",
            "id": format!("{}/activities/{}", from_url, Uuid::new_v4()),
            "actor": actor_url,
            "published": Utc::now().to_rfc3339(),
            "to": ["https://www.w3.org/ns/activitystreams#Public"],
            "object": {
                "@context": "https://www.w3.org/ns/activitystreams",
                "type": "Note",
                "id": note_id.clone(),
                "attributedTo": actor_url,
                "content": content,
                "to": ["https://www.w3.org/ns/activitystreams#Public"],
                "published": Utc::now().to_rfc3339()
            }
        });

        let response = self
            .client
            .post(&outbox_url)
            .header("Content-Type", "application/activity+json")
            .json(&create_activity)
            .send()
            .await
            .map_err(|e| format!("Failed to create note: {}", e))?;

        if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
            info!("{} created note: {}", from_actor, note_id);
            Ok(note_id)
        } else {
            Err(format!("Failed to create note: {}", response.status()))
        }
    }

    async fn check_activity_in_inbox(
        &self,
        inbox_url: &str,
        activity_type: &str,
        timeout: Duration,
    ) -> Result<bool, String> {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            let response = self
                .client
                .get(inbox_url)
                .header("Accept", "application/activity+json")
                .send()
                .await;

            if let Ok(resp) = response {
                if resp.status().is_success() {
                    if let Ok(inbox) = resp.json::<Value>().await {
                        if let Some(items) = inbox
                            .get("orderedItems")
                            .or_else(|| inbox.get("items"))
                            .and_then(|v| v.as_array())
                        {
                            for item in items {
                                if item.get("type").and_then(|v| v.as_str()) == Some(activity_type)
                                {
                                    return Ok(true);
                                }
                            }
                        }
                    }
                }
            }

            sleep(Duration::from_secs(1)).await;
        }

        Ok(false)
    }

    async fn get_followers_collection(&self, actor_url: &str) -> Result<Vec<String>, String> {
        let followers_url = format!("{}/followers", actor_url);

        let response = self
            .client
            .get(&followers_url)
            .header("Accept", "application/activity+json")
            .send()
            .await
            .map_err(|e| format!("Failed to get followers: {}", e))?;

        if !response.status().is_success() {
            return Ok(Vec::new());
        }

        let collection: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse followers collection: {}", e))?;

        let followers = collection
            .get("orderedItems")
            .or_else(|| collection.get("items"))
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(followers)
    }
}

#[tokio::test]
async fn test_follow_accept_workflow() {
    tracing_subscriber::fmt().with_env_filter("debug").init();

    let helper = ActivityPubTestHelper::new();

    info!("Testing Follow/Accept workflow");

    // Wait for services
    helper
        .wait_for_services()
        .await
        .expect("Services failed to start");

    // Create test actors
    let alice_id = helper
        .create_test_actor(&helper.config.solarm_url, "social.solarm.org", "alice")
        .await
        .expect("Failed to create alice");

    let bob_id = helper
        .create_test_actor(&helper.config.space_url, "solarm.space", "bob")
        .await
        .expect("Failed to create bob");

    // Alice follows Bob
    let follow_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/activities/follow-{}", helper.config.solarm_url, Uuid::new_v4()),
        "type": "Follow",
        "actor": alice_id.clone(),
        "object": bob_id.clone(),
        "published": Utc::now().to_rfc3339()
    });

    let follow_id = helper
        .send_follow(&helper.config.solarm_url, "alice", &bob_id)
        .await
        .expect("Failed to send follow");

    info!("Alice sent follow request to Bob: {}", follow_id);

    // Wait for follow to be processed
    sleep(Duration::from_secs(3)).await;

    // Bob accepts the follow
    let accept_id = helper
        .accept_follow(&bob_id, &follow_activity)
        .await
        .expect("Failed to accept follow");

    info!("Bob accepted Alice's follow: {}", accept_id);

    // Wait for acceptance to be processed
    sleep(Duration::from_secs(3)).await;

    // Check if Alice is in Bob's followers
    let followers = helper
        .get_followers_collection(&bob_id)
        .await
        .expect("Failed to get followers");

    assert!(
        followers.contains(&alice_id),
        "Alice should be in Bob's followers after acceptance"
    );

    info!("✅ Follow/Accept workflow test passed");
}

#[tokio::test]
async fn test_follow_reject_workflow() {
    tracing_subscriber::fmt().with_env_filter("debug").init();

    let helper = ActivityPubTestHelper::new();

    info!("Testing Follow/Reject workflow");

    helper
        .wait_for_services()
        .await
        .expect("Services failed to start");

    // Create test actors
    let charlie_id = helper
        .create_test_actor(&helper.config.aopc_url, "social.aopc.cloud", "charlie")
        .await
        .expect("Failed to create charlie");

    let selective_id = helper
        .create_test_actor(&helper.config.solarm_url, "social.solarm.org", "selective")
        .await
        .expect("Failed to create selective user");

    // Charlie follows Selective user
    let follow_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/activities/follow-{}", helper.config.aopc_url, Uuid::new_v4()),
        "type": "Follow",
        "actor": charlie_id.clone(),
        "object": selective_id.clone(),
        "published": Utc::now().to_rfc3339()
    });

    let follow_id = helper
        .send_follow(&helper.config.aopc_url, "charlie", &selective_id)
        .await
        .expect("Failed to send follow");

    info!("Charlie sent follow request to Selective: {}", follow_id);

    sleep(Duration::from_secs(3)).await;

    // Selective user rejects the follow
    let reject_id = helper
        .reject_follow(&selective_id, &follow_activity)
        .await
        .expect("Failed to reject follow");

    info!("Selective rejected Charlie's follow: {}", reject_id);

    sleep(Duration::from_secs(3)).await;

    // Check that Charlie is NOT in Selective's followers
    let followers = helper
        .get_followers_collection(&selective_id)
        .await
        .expect("Failed to get followers");

    assert!(
        !followers.contains(&charlie_id),
        "Charlie should NOT be in Selective's followers after rejection"
    );

    info!("✅ Follow/Reject workflow test passed");
}

#[tokio::test]
async fn test_like_workflow() {
    tracing_subscriber::fmt().with_env_filter("debug").init();

    let helper = ActivityPubTestHelper::new();

    info!("Testing Like workflow");

    helper
        .wait_for_services()
        .await
        .expect("Services failed to start");

    // Create test actors
    let author_id = helper
        .create_test_actor(&helper.config.solarm_url, "social.solarm.org", "author")
        .await
        .expect("Failed to create author");

    let liker1_id = helper
        .create_test_actor(&helper.config.space_url, "solarm.space", "liker1")
        .await
        .expect("Failed to create liker1");

    let liker2_id = helper
        .create_test_actor(&helper.config.aopc_url, "social.aopc.cloud", "liker2")
        .await
        .expect("Failed to create liker2");

    // Author creates a note
    let note_content = format!("This is a test note for likes at {}", Utc::now());
    let note_id = helper
        .create_note(&helper.config.solarm_url, "author", &note_content)
        .await
        .expect("Failed to create note");

    info!("Author created note: {}", note_id);

    sleep(Duration::from_secs(2)).await;

    // Liker1 likes the note
    let like1_id = helper
        .send_like(&helper.config.space_url, "liker1", &note_id)
        .await
        .expect("Failed to send first like");

    info!("Liker1 liked the note: {}", like1_id);

    // Liker2 likes the note
    let like2_id = helper
        .send_like(&helper.config.aopc_url, "liker2", &note_id)
        .await
        .expect("Failed to send second like");

    info!("Liker2 liked the note: {}", like2_id);

    sleep(Duration::from_secs(3)).await;

    // Check if author received the likes
    let author_inbox = format!("{}/inbox", author_id);
    let has_likes = helper
        .check_activity_in_inbox(&author_inbox, "Like", Duration::from_secs(10))
        .await
        .expect("Failed to check for likes");

    assert!(has_likes, "Author should have received Like activities");

    info!("✅ Like workflow test passed");
}

#[tokio::test]
async fn test_announce_workflow() {
    tracing_subscriber::fmt().with_env_filter("debug").init();

    let helper = ActivityPubTestHelper::new();

    info!("Testing Announce (boost/repost) workflow");

    helper
        .wait_for_services()
        .await
        .expect("Services failed to start");

    // Create test actors
    let original_id = helper
        .create_test_actor(&helper.config.solarm_url, "social.solarm.org", "original")
        .await
        .expect("Failed to create original author");

    let booster_id = helper
        .create_test_actor(&helper.config.space_url, "solarm.space", "booster")
        .await
        .expect("Failed to create booster");

    let follower_id = helper
        .create_test_actor(&helper.config.aopc_url, "social.aopc.cloud", "follower")
        .await
        .expect("Failed to create follower");

    // Original author creates a note
    let note_content = format!("Important announcement at {}", Utc::now());
    let note_id = helper
        .create_note(&helper.config.solarm_url, "original", &note_content)
        .await
        .expect("Failed to create note");

    info!("Original author created note: {}", note_id);

    sleep(Duration::from_secs(2)).await;

    // Follower follows Booster (to receive announces)
    let follow_id = helper
        .send_follow(&helper.config.aopc_url, "follower", &booster_id)
        .await
        .expect("Failed to send follow");

    let follow_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": follow_id,
        "type": "Follow",
        "actor": follower_id.clone(),
        "object": booster_id.clone()
    });

    helper
        .accept_follow(&booster_id, &follow_activity)
        .await
        .expect("Failed to accept follow");

    info!("Follower is now following Booster");

    sleep(Duration::from_secs(2)).await;

    // Booster announces (boosts) the note to followers
    let announce_id = helper
        .send_announce(
            &helper.config.space_url,
            "booster",
            &note_id,
            vec![
                follower_id.clone(),
                "https://www.w3.org/ns/activitystreams#Public".to_string(),
            ],
        )
        .await
        .expect("Failed to send announce");

    info!("Booster announced the note: {}", announce_id);

    sleep(Duration::from_secs(5)).await;

    // Check if follower received the announce
    let follower_inbox = format!("{}/inbox", follower_id);
    let has_announce = helper
        .check_activity_in_inbox(&follower_inbox, "Announce", Duration::from_secs(10))
        .await
        .expect("Failed to check for announce");

    assert!(
        has_announce,
        "Follower should have received the Announce activity"
    );

    info!("✅ Announce workflow test passed");
}

#[tokio::test]
async fn test_undo_workflow() {
    tracing_subscriber::fmt().with_env_filter("debug").init();

    let helper = ActivityPubTestHelper::new();

    info!("Testing Undo workflow (Unlike and Unfollow)");

    helper
        .wait_for_services()
        .await
        .expect("Services failed to start");

    // Create test actors
    let user_id = helper
        .create_test_actor(&helper.config.solarm_url, "social.solarm.org", "user")
        .await
        .expect("Failed to create user");

    let target_id = helper
        .create_test_actor(&helper.config.space_url, "solarm.space", "target")
        .await
        .expect("Failed to create target");

    // Test Undo Like
    let note_content = format!("Note for undo test at {}", Utc::now());
    let note_id = helper
        .create_note(&helper.config.space_url, "target", &note_content)
        .await
        .expect("Failed to create note");

    // Send Like
    let like_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/activities/like-{}", helper.config.solarm_url, Uuid::new_v4()),
        "type": "Like",
        "actor": user_id.clone(),
        "object": note_id.clone(),
        "published": Utc::now().to_rfc3339()
    });

    let like_id = helper
        .send_like(&helper.config.solarm_url, "user", &note_id)
        .await
        .expect("Failed to send like");

    info!("User liked the note: {}", like_id);

    sleep(Duration::from_secs(2)).await;

    // Send Undo Like
    let undo_like_id = helper
        .send_undo(&helper.config.solarm_url, "user", &like_activity)
        .await
        .expect("Failed to send undo like");

    info!("User sent undo for like: {}", undo_like_id);

    sleep(Duration::from_secs(3)).await;

    // Test Undo Follow
    let follow_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/activities/follow-{}", helper.config.solarm_url, Uuid::new_v4()),
        "type": "Follow",
        "actor": user_id.clone(),
        "object": target_id.clone(),
        "published": Utc::now().to_rfc3339()
    });

    let follow_id = helper
        .send_follow(&helper.config.solarm_url, "user", &target_id)
        .await
        .expect("Failed to send follow");

    // Accept follow
    helper
        .accept_follow(&target_id, &follow_activity)
        .await
        .expect("Failed to accept follow");

    sleep(Duration::from_secs(3)).await;

    // Check user is in target's followers
    let followers = helper
        .get_followers_collection(&target_id)
        .await
        .expect("Failed to get followers");

    assert!(
        followers.contains(&user_id),
        "User should be in target's followers after follow"
    );

    // Send Undo Follow
    let undo_follow_id = helper
        .send_undo(&helper.config.solarm_url, "user", &follow_activity)
        .await
        .expect("Failed to send undo follow");

    info!("User sent undo for follow: {}", undo_follow_id);

    sleep(Duration::from_secs(3)).await;

    // Check user is no longer in target's followers
    let followers_after = helper
        .get_followers_collection(&target_id)
        .await
        .expect("Failed to get followers after undo");

    assert!(
        !followers_after.contains(&user_id),
        "User should NOT be in target's followers after undo"
    );

    info!("✅ Undo workflow test passed");
}

#[tokio::test]
async fn test_comprehensive_activitypub_workflow() {
    tracing_subscriber::fmt().with_env_filter("debug").init();

    let helper = ActivityPubTestHelper::new();

    info!("Testing comprehensive ActivityPub workflow with multiple activities");

    helper
        .wait_for_services()
        .await
        .expect("Services failed to start");

    // Create a network of test actors
    let alice_id = helper
        .create_test_actor(&helper.config.solarm_url, "social.solarm.org", "alice_comp")
        .await
        .expect("Failed to create alice");

    let bob_id = helper
        .create_test_actor(&helper.config.space_url, "solarm.space", "bob_comp")
        .await
        .expect("Failed to create bob");

    let charlie_id = helper
        .create_test_actor(&helper.config.aopc_url, "social.aopc.cloud", "charlie_comp")
        .await
        .expect("Failed to create charlie");

    info!("Created test network: Alice, Bob, and Charlie");

    // Step 1: Establish follow relationships
    info!("Step 1: Establishing follow relationships");

    // Alice follows Bob
    let alice_follow_bob = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/activities/follow-{}", helper.config.solarm_url, Uuid::new_v4()),
        "type": "Follow",
        "actor": alice_id.clone(),
        "object": bob_id.clone()
    });

    helper
        .send_follow(&helper.config.solarm_url, "alice_comp", &bob_id)
        .await
        .expect("Alice failed to follow Bob");

    helper
        .accept_follow(&bob_id, &alice_follow_bob)
        .await
        .expect("Bob failed to accept Alice's follow");

    // Charlie follows Bob
    let charlie_follow_bob = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/activities/follow-{}", helper.config.aopc_url, Uuid::new_v4()),
        "type": "Follow",
        "actor": charlie_id.clone(),
        "object": bob_id.clone()
    });

    helper
        .send_follow(&helper.config.aopc_url, "charlie_comp", &bob_id)
        .await
        .expect("Charlie failed to follow Bob");

    helper
        .accept_follow(&bob_id, &charlie_follow_bob)
        .await
        .expect("Bob failed to accept Charlie's follow");

    // Bob follows Alice (mutual follow)
    let bob_follow_alice = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/activities/follow-{}", helper.config.space_url, Uuid::new_v4()),
        "type": "Follow",
        "actor": bob_id.clone(),
        "object": alice_id.clone()
    });

    helper
        .send_follow(&helper.config.space_url, "bob_comp", &alice_id)
        .await
        .expect("Bob failed to follow Alice");

    helper
        .accept_follow(&alice_id, &bob_follow_alice)
        .await
        .expect("Alice failed to accept Bob's follow");

    info!("Follow relationships established: Alice <-> Bob <- Charlie");

    sleep(Duration::from_secs(3)).await;

    // Step 2: Bob creates a note
    info!("Step 2: Bob creates a note");

    let bob_note_content = format!("Hello from Bob! This is a test note at {}", Utc::now());
    let bob_note_id = helper
        .create_note(&helper.config.space_url, "bob_comp", &bob_note_content)
        .await
        .expect("Bob failed to create note");

    info!("Bob created note: {}", bob_note_id);

    sleep(Duration::from_secs(2)).await;

    // Step 3: Alice and Charlie like Bob's note
    info!("Step 3: Multiple actors like Bob's note");

    let alice_like_id = helper
        .send_like(&helper.config.solarm_url, "alice_comp", &bob_note_id)
        .await
        .expect("Alice failed to like Bob's note");

    let charlie_like_id = helper
        .send_like(&helper.config.aopc_url, "charlie_comp", &bob_note_id)
        .await
        .expect("Charlie failed to like Bob's note");

    info!("Alice and Charlie liked Bob's note");

    sleep(Duration::from_secs(2)).await;

    // Step 4: Alice announces (boosts) Bob's note
    info!("Step 4: Alice announces Bob's note to her followers");

    let alice_announce_id = helper
        .send_announce(
            &helper.config.solarm_url,
            "alice_comp",
            &bob_note_id,
            vec![
                "https://www.w3.org/ns/activitystreams#Public".to_string(),
                format!("{}/followers", alice_id),
            ],
        )
        .await
        .expect("Alice failed to announce Bob's note");

    info!("Alice announced Bob's note: {}", alice_announce_id);

    sleep(Duration::from_secs(3)).await;

    // Step 5: Charlie creates a reply to Bob's note
    info!("Step 5: Charlie replies to Bob's note");

    let charlie_reply_content = format!("@bob_comp Great note! Here's my reply at {}", Utc::now());

    let charlie_reply = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "id": format!("{}/activities/{}", helper.config.aopc_url, Uuid::new_v4()),
        "actor": charlie_id.clone(),
        "published": Utc::now().to_rfc3339(),
        "to": [bob_id.clone(), "https://www.w3.org/ns/activitystreams#Public"],
        "object": {
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Note",
            "id": format!("{}/users/charlie_comp/notes/{}", helper.config.aopc_url, Uuid::new_v4()),
            "attributedTo": charlie_id.clone(),
            "content": charlie_reply_content,
            "inReplyTo": bob_note_id.clone(),
            "to": [bob_id.clone(), "https://www.w3.org/ns/activitystreams#Public"],
            "published": Utc::now().to_rfc3339()
        }
    });

    let response = helper
        .client
        .post(format!(
            "{}/users/charlie_comp/outbox",
            helper.config.aopc_url
        ))
        .header("Content-Type", "application/activity+json")
        .json(&charlie_reply)
        .send()
        .await
        .expect("Failed to send reply");

    assert!(
        response.status().is_success() || response.status() == StatusCode::ACCEPTED,
        "Failed to create reply"
    );

    info!("Charlie replied to Bob's note");

    sleep(Duration::from_secs(3)).await;

    // Step 6: Test different response scenarios
    info!("Step 6: Testing mixed follow scenarios");

    // New actor attempts to follow Bob but gets rejected
    let david_id = helper
        .create_test_actor(&helper.config.solarm_url, "social.solarm.org", "david_comp")
        .await
        .expect("Failed to create david");

    let david_follow = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": format!("{}/activities/follow-{}", helper.config.solarm_url, Uuid::new_v4()),
        "type": "Follow",
        "actor": david_id.clone(),
        "object": bob_id.clone()
    });

    helper
        .send_follow(&helper.config.solarm_url, "david_comp", &bob_id)
        .await
        .expect("David failed to send follow");

    // Bob rejects David's follow
    helper
        .reject_follow(&bob_id, &david_follow)
        .await
        .expect("Bob failed to reject David's follow");

    info!("Bob rejected David's follow request");

    sleep(Duration::from_secs(2)).await;

    // Verify relationships
    let bob_followers = helper
        .get_followers_collection(&bob_id)
        .await
        .expect("Failed to get Bob's followers");

    assert!(
        bob_followers.contains(&alice_id),
        "Alice should be in Bob's followers"
    );
    assert!(
        bob_followers.contains(&charlie_id),
        "Charlie should be in Bob's followers"
    );
    assert!(
        !bob_followers.contains(&david_id),
        "David should NOT be in Bob's followers"
    );

    // Step 7: Test Undo scenario
    info!("Step 7: Charlie unlikes and unfollows");

    // Charlie unlikes Bob's note
    let charlie_like = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "id": charlie_like_id,
        "type": "Like",
        "actor": charlie_id.clone(),
        "object": bob_note_id.clone()
    });

    helper
        .send_undo(&helper.config.aopc_url, "charlie_comp", &charlie_like)
        .await
        .expect("Charlie failed to undo like");

    // Charlie unfollows Bob
    helper
        .send_undo(&helper.config.aopc_url, "charlie_comp", &charlie_follow_bob)
        .await
        .expect("Charlie failed to unfollow Bob");

    info!("Charlie unliked Bob's note and unfollowed Bob");

    sleep(Duration::from_secs(3)).await;

    // Final verification
    let final_bob_followers = helper
        .get_followers_collection(&bob_id)
        .await
        .expect("Failed to get Bob's final followers");

    assert!(
        final_bob_followers.contains(&alice_id),
        "Alice should still be in Bob's followers"
    );
    assert!(
        !final_bob_followers.contains(&charlie_id),
        "Charlie should NO LONGER be in Bob's followers after unfollow"
    );

    info!("✅ Comprehensive ActivityPub workflow test passed!");
    info!("Successfully tested: Follow, Accept, Reject, Like, Announce, Reply, and Undo");
}
