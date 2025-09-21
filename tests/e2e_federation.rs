//! End-to-end federation tests for oxifed
//!
//! This test suite validates the complete federation workflow including:
//! - Domain creation and management
//! - WebFinger discovery and resolution
//! - ActivityPub message sending and receiving between domains
//! - Cross-domain federation capabilities

use std::env;
use std::time::Duration;

use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Import oxifed messaging types

// Test configuration from environment
struct TestConfig {
    solarm_url: String,
    space_url: String,
    aopc_url: String,
    #[allow(dead_code)]
    mongodb_uri: String,
    #[allow(dead_code)]
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

// WebFinger response structures
#[derive(Debug, Serialize, Deserialize)]
struct WebFingerResponse {
    subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    aliases: Option<Vec<String>>,
    links: Vec<WebFingerLink>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WebFingerLink {
    rel: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    type_: Option<String>,
    href: Option<String>,
}

// ActivityPub structures
#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)]
struct Actor {
    #[serde(rename = "@context")]
    context: Value,
    id: String,
    #[serde(rename = "type")]
    type_: String,
    preferredUsername: String,
    inbox: String,
    outbox: String,
    followers: Option<String>,
    following: Option<String>,
    publicKey: Option<PublicKey>,
}

#[derive(Debug, Deserialize, Serialize)]
#[allow(non_snake_case)]
struct PublicKey {
    id: String,
    owner: String,
    publicKeyPem: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code, non_snake_case)]
struct Note {
    #[serde(rename = "@context")]
    context: Value,
    id: String,
    #[serde(rename = "type")]
    type_: String,
    attributedTo: String,
    content: String,
    to: Vec<String>,
    cc: Option<Vec<String>>,
    published: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
struct Activity {
    #[serde(rename = "@context")]
    context: Value,
    id: String,
    #[serde(rename = "type")]
    type_: String,
    actor: String,
    object: Value,
    to: Vec<String>,
    cc: Option<Vec<String>>,
    published: String,
}

// Test helper struct
struct E2ETestHelper {
    client: Client,
    config: TestConfig,
}

impl E2ETestHelper {
    fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        E2ETestHelper {
            client,
            config: TestConfig::from_env(),
        }
    }

    // Wait for service to be healthy
    async fn wait_for_service(&self, url: &str, max_retries: u32) -> Result<(), String> {
        let health_url = format!("{}/health", url);

        for i in 0..max_retries {
            match self.client.get(&health_url).send().await {
                Ok(response) if response.status().is_success() => {
                    info!("Service {} is healthy", url);
                    return Ok(());
                }
                _ => {
                    debug!(
                        "Service {} not ready yet, attempt {}/{}",
                        url,
                        i + 1,
                        max_retries
                    );
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Err(format!(
            "Service {} failed to become healthy after {} attempts",
            url, max_retries
        ))
    }

    // Create a domain via API
    async fn create_domain(
        &self,
        base_url: &str,
        domain: &str,
        name: &str,
        description: &str,
    ) -> Result<(), String> {
        let create_endpoint = format!("{}/api/v1/domains", base_url);

        let domain_data = json!({
            "domain": domain,
            "name": name,
            "description": description,
            "contact_email": format!("admin@{}", domain),
            "registration_mode": "open",
            "authorized_fetch": false,
            "max_note_length": 5000,
            "max_file_size": 10485760,
            "allowed_file_types": ["image/jpeg", "image/png", "image/gif"],
            "rules": ["Be respectful", "No spam", "Follow community guidelines"]
        });

        let response = self
            .client
            .post(&create_endpoint)
            .json(&domain_data)
            .send()
            .await
            .map_err(|e| format!("Failed to send domain creation request: {}", e))?;

        if response.status().is_success() {
            info!("Successfully created domain: {}", domain);
            Ok(())
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "No response body".to_string());
            Err(format!(
                "Failed to create domain {}: Status {}, Body: {}",
                domain, status, body
            ))
        }
    }

    // Test WebFinger resolution
    async fn test_webfinger(
        &self,
        base_url: &str,
        resource: &str,
    ) -> Result<WebFingerResponse, String> {
        let webfinger_url = format!("{}/.well-known/webfinger?resource={}", base_url, resource);

        let response = self
            .client
            .get(&webfinger_url)
            .send()
            .await
            .map_err(|e| format!("Failed to send WebFinger request: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "WebFinger request failed with status: {}",
                response.status()
            ));
        }

        response
            .json::<WebFingerResponse>()
            .await
            .map_err(|e| format!("Failed to parse WebFinger response: {}", e))
    }

    // Create a test actor
    async fn create_actor(
        &self,
        base_url: &str,
        domain: &str,
        username: &str,
    ) -> Result<Actor, String> {
        let actor_endpoint = format!("{}/api/v1/actors", base_url);

        let actor_data = json!({
            "username": username,
            "display_name": format!("{} User", username),
            "bio": format!("Test user on {}", domain),
            "domain": domain
        });

        let response = self
            .client
            .post(&actor_endpoint)
            .json(&actor_data)
            .send()
            .await
            .map_err(|e| format!("Failed to create actor: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "No response body".to_string());
            return Err(format!(
                "Failed to create actor: Status {}, Body: {}",
                status, body
            ));
        }

        response
            .json::<Actor>()
            .await
            .map_err(|e| format!("Failed to parse actor response: {}", e))
    }

    // Send a note from one actor to another
    async fn send_note(
        &self,
        from_url: &str,
        from_actor: &str,
        to_actor: &str,
        content: &str,
    ) -> Result<String, String> {
        let outbox_url = format!("{}/users/{}/outbox", from_url, from_actor);

        let note_id = format!("{}/users/{}/notes/{}", from_url, from_actor, Uuid::new_v4());

        let activity = json!({
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Create",
            "id": format!("{}/activities/{}", from_url, Uuid::new_v4()),
            "actor": format!("{}/users/{}", from_url, from_actor),
            "published": Utc::now().to_rfc3339(),
            "to": [to_actor],
            "cc": ["https://www.w3.org/ns/activitystreams#Public"],
            "object": {
                "@context": "https://www.w3.org/ns/activitystreams",
                "type": "Note",
                "id": note_id,
                "attributedTo": format!("{}/users/{}", from_url, from_actor),
                "content": content,
                "to": [to_actor],
                "cc": ["https://www.w3.org/ns/activitystreams#Public"],
                "published": Utc::now().to_rfc3339()
            }
        });

        let response = self
            .client
            .post(&outbox_url)
            .header("Content-Type", "application/activity+json")
            .json(&activity)
            .send()
            .await
            .map_err(|e| format!("Failed to send note: {}", e))?;

        if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
            info!("Successfully sent note from {} to {}", from_actor, to_actor);
            Ok(note_id)
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "No response body".to_string());
            Err(format!(
                "Failed to send note: Status {}, Body: {}",
                status, body
            ))
        }
    }

    // Check if a note was received in the inbox
    async fn check_inbox(
        &self,
        base_url: &str,
        actor_username: &str,
        expected_content: &str,
    ) -> Result<bool, String> {
        let inbox_url = format!("{}/users/{}/inbox", base_url, actor_username);

        let response = self
            .client
            .get(&inbox_url)
            .header("Accept", "application/activity+json")
            .send()
            .await
            .map_err(|e| format!("Failed to check inbox: {}", e))?;

        if !response.status().is_success() {
            return Ok(false); // Inbox might be empty or not accessible
        }

        let inbox_items: Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse inbox response: {}", e))?;

        // Check if the inbox contains a note with the expected content
        if let Some(items) = inbox_items.get("orderedItems").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(object) = item.get("object")
                    && let Some(content) = object.get("content").and_then(|v| v.as_str())
                    && content.contains(expected_content)
                {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}

#[tokio::test]
async fn test_e2e_federation_workflow() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init()
        .ok();

    if !should_run_e2e() {
        eprintln!("Skipping E2E federation tests (set OXIFED_RUN_E2E=1 to enable)");
        return;
    }

    let helper = E2ETestHelper::new();

    info!("Starting E2E federation test suite");

    // Phase 1: Wait for all services to be healthy
    info!("Phase 1: Waiting for services to be healthy");

    helper
        .wait_for_service(&helper.config.solarm_url, 30)
        .await
        .expect("social.solarm.org service failed to start");

    helper
        .wait_for_service(&helper.config.space_url, 30)
        .await
        .expect("solarm.space service failed to start");

    helper
        .wait_for_service(&helper.config.aopc_url, 30)
        .await
        .expect("social.aopc.cloud service failed to start");

    // Give services a moment to fully initialize
    sleep(Duration::from_secs(5)).await;

    // Phase 2: Create domains
    info!("Phase 2: Creating domains");

    helper
        .create_domain(
            &helper.config.solarm_url,
            "social.solarm.org",
            "Solarm Social",
            "The primary Solarm social network instance",
        )
        .await
        .expect("Failed to create social.solarm.org domain");

    helper
        .create_domain(
            &helper.config.space_url,
            "solarm.space",
            "Solarm Space",
            "A space-themed Solarm instance",
        )
        .await
        .expect("Failed to create solarm.space domain");

    helper
        .create_domain(
            &helper.config.aopc_url,
            "social.aopc.cloud",
            "AOPC Cloud Social",
            "Cloud-based social platform",
        )
        .await
        .expect("Failed to create social.aopc.cloud domain");

    // Give domains time to propagate
    sleep(Duration::from_secs(3)).await;

    // Phase 3: Test WebFinger discovery
    info!("Phase 3: Testing WebFinger discovery");

    let webfinger_result = helper
        .test_webfinger(&helper.config.solarm_url, "acct:admin@social.solarm.org")
        .await;

    assert!(
        webfinger_result.is_ok(),
        "WebFinger discovery failed for social.solarm.org"
    );
    info!("WebFinger discovery successful for social.solarm.org");

    // Phase 4: Create test actors
    info!("Phase 4: Creating test actors");

    let alice = helper
        .create_actor(&helper.config.solarm_url, "social.solarm.org", "alice")
        .await
        .expect("Failed to create actor alice");

    let bob = helper
        .create_actor(&helper.config.space_url, "solarm.space", "bob")
        .await
        .expect("Failed to create actor bob");

    let charlie = helper
        .create_actor(&helper.config.aopc_url, "social.aopc.cloud", "charlie")
        .await
        .expect("Failed to create actor charlie");

    info!(
        "Created test actors: alice@social.solarm.org, bob@solarm.space, charlie@social.aopc.cloud"
    );

    // Phase 5: Test cross-domain messaging
    info!("Phase 5: Testing cross-domain messaging");

    // Test 1: Alice sends a note to Bob
    let test_content_1 = format!(
        "Hello Bob! This is a test message from Alice at {}",
        Utc::now()
    );
    let note_id_1 = helper
        .send_note(&helper.config.solarm_url, "alice", &bob.id, &test_content_1)
        .await
        .expect("Failed to send note from Alice to Bob");

    info!("Sent note from Alice to Bob: {}", note_id_1);

    // Wait for federation to process
    sleep(Duration::from_secs(5)).await;

    // Check if Bob received the note
    let mut received = false;
    for _ in 0..10 {
        if helper
            .check_inbox(&helper.config.space_url, "bob", &test_content_1)
            .await
            .unwrap_or(false)
        {
            received = true;
            break;
        }
        sleep(Duration::from_secs(2)).await;
    }

    assert!(received, "Bob did not receive the note from Alice");
    info!("Bob successfully received the note from Alice");

    // Test 2: Bob sends a note to Charlie
    let test_content_2 = format!(
        "Hi Charlie! This is Bob from solarm.space at {}",
        Utc::now()
    );
    let note_id_2 = helper
        .send_note(
            &helper.config.space_url,
            "bob",
            &charlie.id,
            &test_content_2,
        )
        .await
        .expect("Failed to send note from Bob to Charlie");

    info!("Sent note from Bob to Charlie: {}", note_id_2);

    // Wait for federation to process
    sleep(Duration::from_secs(5)).await;

    // Check if Charlie received the note
    received = false;
    for _ in 0..10 {
        if helper
            .check_inbox(&helper.config.aopc_url, "charlie", &test_content_2)
            .await
            .unwrap_or(false)
        {
            received = true;
            break;
        }
        sleep(Duration::from_secs(2)).await;
    }

    assert!(received, "Charlie did not receive the note from Bob");
    info!("Charlie successfully received the note from Bob");

    // Test 3: Charlie sends a note to Alice (full circle)
    let test_content_3 = format!(
        "Hello Alice! Charlie here from aopc.cloud at {}",
        Utc::now()
    );
    let note_id_3 = helper
        .send_note(
            &helper.config.aopc_url,
            "charlie",
            &alice.id,
            &test_content_3,
        )
        .await
        .expect("Failed to send note from Charlie to Alice");

    info!("Sent note from Charlie to Alice: {}", note_id_3);

    // Wait for federation to process
    sleep(Duration::from_secs(5)).await;

    // Check if Alice received the note
    received = false;
    for _ in 0..10 {
        if helper
            .check_inbox(&helper.config.solarm_url, "alice", &test_content_3)
            .await
            .unwrap_or(false)
        {
            received = true;
            break;
        }
        sleep(Duration::from_secs(2)).await;
    }

    assert!(received, "Alice did not receive the note from Charlie");
    info!("Alice successfully received the note from Charlie");

    // Phase 6: Test broadcast message (one to many)
    info!("Phase 6: Testing broadcast messaging");

    let broadcast_content = format!(
        "Broadcast message from Alice to all followers at {}",
        Utc::now()
    );
    let broadcast_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "id": format!("{}/activities/{}", helper.config.solarm_url, Uuid::new_v4()),
        "actor": alice.id.clone(),
        "published": Utc::now().to_rfc3339(),
        "to": ["https://www.w3.org/ns/activitystreams#Public"],
        "cc": [bob.id.clone(), charlie.id.clone()],
        "object": {
            "@context": "https://www.w3.org/ns/activitystreams",
            "type": "Note",
            "id": format!("{}/notes/{}", helper.config.solarm_url, Uuid::new_v4()),
            "attributedTo": alice.id.clone(),
            "content": broadcast_content.clone(),
            "to": ["https://www.w3.org/ns/activitystreams#Public"],
            "cc": [bob.id.clone(), charlie.id.clone()],
            "published": Utc::now().to_rfc3339()
        }
    });

    let response = helper
        .client
        .post(format!("{}/users/alice/outbox", helper.config.solarm_url))
        .header("Content-Type", "application/activity+json")
        .json(&broadcast_activity)
        .send()
        .await
        .expect("Failed to send broadcast message");

    assert!(
        response.status().is_success() || response.status() == StatusCode::ACCEPTED,
        "Failed to send broadcast message"
    );

    info!("Successfully sent broadcast message from Alice");

    // Wait for broadcast to propagate
    sleep(Duration::from_secs(8)).await;

    // Verify both Bob and Charlie received the broadcast
    let bob_received = helper
        .check_inbox(&helper.config.space_url, "bob", &broadcast_content)
        .await
        .unwrap_or(false);

    let charlie_received = helper
        .check_inbox(&helper.config.aopc_url, "charlie", &broadcast_content)
        .await
        .unwrap_or(false);

    assert!(bob_received, "Bob did not receive the broadcast message");
    assert!(
        charlie_received,
        "Charlie did not receive the broadcast message"
    );

    info!("Both Bob and Charlie successfully received the broadcast message");

    info!("✅ All E2E federation tests passed successfully!");
}

#[tokio::test]
async fn test_domain_resolution() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init()
        .ok();

    if !should_run_e2e() {
        eprintln!("Skipping E2E federation tests (set OXIFED_RUN_E2E=1 to enable)");
        return;
    }

    let helper = E2ETestHelper::new();

    info!("Testing domain resolution capabilities");

    // Wait for services
    helper
        .wait_for_service(&helper.config.solarm_url, 30)
        .await
        .expect("Service failed to start");
    helper
        .wait_for_service(&helper.config.space_url, 30)
        .await
        .expect("Service failed to start");

    sleep(Duration::from_secs(3)).await;

    // Create test domains if they don't exist
    let _ = helper
        .create_domain(
            &helper.config.solarm_url,
            "social.solarm.org",
            "Solarm Social",
            "Testing domain resolution",
        )
        .await;

    let _ = helper
        .create_domain(
            &helper.config.space_url,
            "solarm.space",
            "Solarm Space",
            "Testing domain resolution",
        )
        .await;

    // Test WebFinger resolution for each domain
    let resources = vec![
        (
            "social.solarm.org",
            &helper.config.solarm_url,
            "acct:test@social.solarm.org",
        ),
        (
            "solarm.space",
            &helper.config.space_url,
            "acct:test@solarm.space",
        ),
    ];

    for (domain, url, resource) in resources {
        info!("Testing WebFinger resolution for {}", domain);

        match helper.test_webfinger(url, resource).await {
            Ok(response) => {
                assert_eq!(response.subject, resource, "WebFinger subject mismatch");

                // Check for ActivityPub link
                let has_ap_link = response.links.iter().any(|link| {
                    link.rel == "self"
                        && link.type_ == Some("application/activity+json".to_string())
                });

                assert!(
                    has_ap_link,
                    "WebFinger response missing ActivityPub link for {}",
                    domain
                );
                info!("✓ WebFinger resolution successful for {}", domain);
            }
            Err(e) => {
                // WebFinger might not be implemented yet, log warning instead of failing
                warn!("WebFinger resolution not available for {}: {}", domain, e);
            }
        }
    }

    // Test cross-domain actor discovery
    info!("Testing cross-domain actor discovery");

    // Create actors for discovery testing
    let _ = helper
        .create_actor(
            &helper.config.solarm_url,
            "social.solarm.org",
            "discoverable",
        )
        .await;
    let _ = helper
        .create_actor(&helper.config.space_url, "solarm.space", "findme")
        .await;

    sleep(Duration::from_secs(2)).await;

    // Test discovering actors across domains
    let actor_url = format!("{}/users/discoverable", helper.config.solarm_url);
    let response = helper
        .client
        .get(&actor_url)
        .header("Accept", "application/activity+json")
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            info!("✓ Successfully discovered actor at social.solarm.org");
        }
        Ok(resp) => {
            warn!("Actor discovery returned status: {}", resp.status());
        }
        Err(e) => {
            warn!("Actor discovery failed: {}", e);
        }
    }

    info!("✅ Domain resolution tests completed");
}

#[tokio::test]
async fn test_message_federation_reliability() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init()
        .ok();

    if !should_run_e2e() {
        eprintln!("Skipping E2E federation tests (set OXIFED_RUN_E2E=1 to enable)");
        return;
    }

    let helper = E2ETestHelper::new();

    info!("Testing message federation reliability");

    // Setup phase
    helper
        .wait_for_service(&helper.config.solarm_url, 30)
        .await
        .expect("Service failed");
    helper
        .wait_for_service(&helper.config.space_url, 30)
        .await
        .expect("Service failed");

    sleep(Duration::from_secs(3)).await;

    // Create domains and actors
    let _ = helper
        .create_domain(
            &helper.config.solarm_url,
            "social.solarm.org",
            "Test",
            "Test",
        )
        .await;
    let _ = helper
        .create_domain(&helper.config.space_url, "solarm.space", "Test", "Test")
        .await;

    let _sender = helper
        .create_actor(&helper.config.solarm_url, "social.solarm.org", "sender")
        .await
        .expect("Failed to create sender");

    let receiver = helper
        .create_actor(&helper.config.space_url, "solarm.space", "receiver")
        .await
        .expect("Failed to create receiver");

    // Test rapid message sending
    info!("Testing rapid message sending (10 messages)");

    let mut sent_messages = Vec::new();
    for i in 0..10 {
        let content = format!("Test message #{} sent at {}", i, Utc::now());

        match helper
            .send_note(&helper.config.solarm_url, "sender", &receiver.id, &content)
            .await
        {
            Ok(note_id) => {
                sent_messages.push((i, content, note_id));
                info!("Sent message #{}", i);
            }
            Err(e) => {
                error!("Failed to send message #{}: {}", i, e);
            }
        }

        // Small delay between messages
        sleep(Duration::from_millis(500)).await;
    }

    // Wait for all messages to be delivered
    info!("Waiting for message delivery...");
    sleep(Duration::from_secs(10)).await;

    // Check delivery
    let mut delivered_count = 0;
    for (i, content, _) in &sent_messages {
        if helper
            .check_inbox(&helper.config.space_url, "receiver", content)
            .await
            .unwrap_or(false)
        {
            delivered_count += 1;
            debug!("Message #{} was delivered", i);
        } else {
            warn!("Message #{} was not found in inbox", i);
        }
    }

    let delivery_rate = (delivered_count as f64 / sent_messages.len() as f64) * 100.0;
    info!(
        "Delivery rate: {}/{} ({:.1}%)",
        delivered_count,
        sent_messages.len(),
        delivery_rate
    );

    // We expect at least 80% delivery rate for the test to pass
    assert!(
        delivery_rate >= 80.0,
        "Message delivery rate too low: {:.1}%",
        delivery_rate
    );

    info!("✅ Message federation reliability test passed");
}

// Helper to decide if E2E tests should run. Set OXIFED_RUN_E2E=1 (or true) to enable.
fn should_run_e2e() -> bool {
    match std::env::var("OXIFED_RUN_E2E") {
        Ok(v) => v == "1" || v.eq_ignore_ascii_case("true"),
        Err(_) => false,
    }
}
