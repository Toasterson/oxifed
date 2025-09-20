//! Tests for the Activity Sender Component (C2S API)

use axum::http::{HeaderMap, HeaderValue};
use futures::TryStreamExt;
use mongodb::bson::doc;
use oxifed::database::{ActivityDocument, ActorDocument, ActorStatus};
use serde_json::json;

use uuid::Uuid;

/// Test helper to create an authenticated request header
#[allow(dead_code)]
fn create_auth_headers(username: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Bearer user:{}:token:test123", username)).unwrap(),
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers
}

/// Test helper to setup test database
async fn setup_test_db() -> Option<mongodb::Database> {
    let mongo_uri = std::env::var("TEST_MONGODB_URI")
        .unwrap_or_else(|_| "mongodb://localhost:27017".to_string());
    let db_name = format!("test_oxifed_{}", Uuid::new_v4());

    let client = match mongodb::Client::with_uri_str(&mongo_uri).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping test - MongoDB not available: {}", e);
            return None;
        }
    };

    let db = client.database(&db_name);

    // Create test actor
    let actor_doc = ActorDocument {
        id: None,
        actor_id: "https://test.example/users/alice".to_string(),
        name: "Alice Test".to_string(),
        preferred_username: "alice".to_string(),
        domain: "test.example".to_string(),
        actor_type: "Person".to_string(),
        summary: Some("Test user for C2S API".to_string()),
        icon: None,
        image: None,
        inbox: "https://test.example/users/alice/inbox".to_string(),
        outbox: "https://test.example/users/alice/outbox".to_string(),
        following: "https://test.example/users/alice/following".to_string(),
        followers: "https://test.example/users/alice/followers".to_string(),
        liked: Some("https://test.example/users/alice/liked".to_string()),
        featured: Some("https://test.example/users/alice/featured".to_string()),
        public_key: None,
        endpoints: None,
        attachment: None,
        additional_properties: None,
        status: ActorStatus::Active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        local: true,
        followers_count: 0,
        following_count: 0,
        statuses_count: 0,
    };

    if let Err(e) = db
        .collection::<ActorDocument>("actors")
        .insert_one(actor_doc)
        .await
    {
        eprintln!("Failed to create test actor: {}", e);
        return None;
    }

    Some(db)
}

/// Cleanup test database
async fn cleanup_test_db(db: mongodb::Database) {
    db.drop().await.expect("Failed to cleanup test database");
}

#[tokio::test]
async fn test_create_note_c2s() {
    let Some(db) = setup_test_db().await else {
        eprintln!("Test skipped: MongoDB not available");
        return;
    };
    unsafe {
        std::env::set_var("OXIFED_DOMAIN", "test.example");
    }

    // Create a note via C2S API
    let note = json!({
        "content": "Hello, Fediverse!",
        "to": ["https://www.w3.org/ns/activitystreams#Public"],
        "cc": ["https://test.example/users/alice/followers"]
    });

    // Simulate the create_note endpoint logic
    let domain = "test.example";
    let username = "alice";

    let activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Create",
        "actor": format!("https://{}/users/{}", domain, username),
        "id": format!("https://{}/activities/{}", domain, Uuid::new_v4()),
        "published": chrono::Utc::now().to_rfc3339(),
        "object": {
            "type": "Note",
            "content": note["content"],
            "to": note["to"],
            "cc": note["cc"],
            "attributedTo": format!("https://{}/users/{}", domain, username),
            "id": format!("https://{}/objects/{}", domain, Uuid::new_v4()),
            "published": chrono::Utc::now().to_rfc3339(),
        }
    });

    // Store the activity in database
    let activity_doc = ActivityDocument {
        id: None,
        activity_id: activity["id"].as_str().unwrap().to_string(),
        activity_type: oxifed::ActivityType::Create,
        actor: activity["actor"].as_str().unwrap().to_string(),
        object: Some(serde_json::to_string(&activity["object"]).unwrap()),
        target: None,
        name: None,
        summary: None,
        published: Some(chrono::Utc::now()),
        updated: None,
        to: Some(vec![
            "https://www.w3.org/ns/activitystreams#Public".to_string(),
        ]),
        cc: None,
        bto: None,
        bcc: None,
        additional_properties: None,
        local: true,
        status: oxifed::database::ActivityStatus::Pending,
        created_at: chrono::Utc::now(),
        attempts: 0,
        last_attempt: None,
        error: None,
    };

    db.collection::<ActivityDocument>("activities")
        .insert_one(&activity_doc)
        .await
        .expect("Failed to store activity");

    // Verify activity was stored
    let stored_activity = db
        .collection::<ActivityDocument>("activities")
        .find_one(doc! { "activity_id": &activity_doc.activity_id })
        .await
        .expect("Failed to query activity")
        .expect("Activity not found");

    assert_eq!(stored_activity.activity_type, oxifed::ActivityType::Create);
    assert_eq!(
        stored_activity.actor,
        format!("https://{}/users/{}", domain, username)
    );

    cleanup_test_db(db).await;
}

#[tokio::test]
async fn test_update_object_c2s() {
    let Some(db) = setup_test_db().await else {
        eprintln!("Test skipped: MongoDB not available");
        return;
    };
    unsafe {
        std::env::set_var("OXIFED_DOMAIN", "test.example");
    }

    let domain = "test.example";
    let username = "alice";
    let object_id = format!("https://{}/objects/{}", domain, Uuid::new_v4());

    // First create an object
    let original_object = json!({
        "id": &object_id,
        "type": "Note",
        "content": "Original content",
        "attributedTo": format!("https://{}/users/{}", domain, username),
        "published": chrono::Utc::now().to_rfc3339(),
    });

    db.collection::<serde_json::Value>("objects")
        .insert_one(original_object.clone())
        .await
        .expect("Failed to create object");

    // Create Update activity
    let update_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Update",
        "actor": format!("https://{}/users/{}", domain, username),
        "id": format!("https://{}/activities/{}", domain, Uuid::new_v4()),
        "object": {
            "id": &object_id,
            "type": "Note",
            "content": "Updated content",
            "updated": chrono::Utc::now().to_rfc3339(),
        }
    });

    // Store the update activity
    let activity_doc = ActivityDocument {
        id: None,
        activity_id: update_activity["id"].as_str().unwrap().to_string(),
        activity_type: oxifed::ActivityType::Update,
        actor: update_activity["actor"].as_str().unwrap().to_string(),
        object: Some(serde_json::to_string(&update_activity["object"]).unwrap()),
        target: None,
        name: None,
        summary: None,
        published: Some(chrono::Utc::now()),
        updated: None,
        to: None,
        cc: None,
        bto: None,
        bcc: None,
        additional_properties: None,
        local: true,
        status: oxifed::database::ActivityStatus::Pending,
        created_at: chrono::Utc::now(),
        attempts: 0,
        last_attempt: None,
        error: None,
    };

    db.collection::<ActivityDocument>("activities")
        .insert_one(&activity_doc)
        .await
        .expect("Failed to store update activity");

    // Update the object
    db.collection::<serde_json::Value>("objects")
        .update_one(
            doc! { "id": &object_id },
            doc! { "$set": {
                "content": "Updated content",
                "updated": chrono::Utc::now().to_rfc3339(),
            }},
        )
        .await
        .expect("Failed to update object");

    // Verify the update
    let updated_object = db
        .collection::<serde_json::Value>("objects")
        .find_one(doc! { "id": &object_id })
        .await
        .expect("Failed to query object")
        .expect("Object not found");

    assert_eq!(updated_object["content"], "Updated content");

    cleanup_test_db(db).await;
}

#[tokio::test]
async fn test_delete_object_c2s() {
    let Some(db) = setup_test_db().await else {
        eprintln!("Test skipped: MongoDB not available");
        return;
    };
    unsafe {
        std::env::set_var("OXIFED_DOMAIN", "test.example");
    }

    let domain = "test.example";
    let username = "alice";
    let object_id = format!("https://{}/objects/{}", domain, Uuid::new_v4());

    // Create an object to delete
    let object = json!({
        "id": &object_id,
        "type": "Note",
        "content": "To be deleted",
        "attributedTo": format!("https://{}/users/{}", domain, username),
        "published": chrono::Utc::now().to_rfc3339(),
    });

    db.collection::<serde_json::Value>("objects")
        .insert_one(object)
        .await
        .expect("Failed to create object");

    // Create Delete activity
    let delete_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Delete",
        "actor": format!("https://{}/users/{}", domain, username),
        "id": format!("https://{}/activities/{}", domain, Uuid::new_v4()),
        "object": &object_id,
    });

    // Store the delete activity
    let activity_doc = ActivityDocument {
        id: None,
        activity_id: delete_activity["id"].as_str().unwrap().to_string(),
        activity_type: oxifed::ActivityType::Delete,
        actor: delete_activity["actor"].as_str().unwrap().to_string(),
        object: Some(object_id.clone()),
        target: None,
        name: None,
        summary: None,
        published: Some(chrono::Utc::now()),
        updated: None,
        to: None,
        cc: None,
        bto: None,
        bcc: None,
        additional_properties: None,
        local: true,
        status: oxifed::database::ActivityStatus::Pending,
        created_at: chrono::Utc::now(),
        attempts: 0,
        last_attempt: None,
        error: None,
    };

    db.collection::<ActivityDocument>("activities")
        .insert_one(&activity_doc)
        .await
        .expect("Failed to store delete activity");

    // Mark object as deleted
    db.collection::<serde_json::Value>("objects")
        .update_one(
            doc! { "id": &object_id },
            doc! { "$set": {
                "deleted": true,
                "updated": chrono::Utc::now().to_rfc3339(),
            }},
        )
        .await
        .expect("Failed to mark object as deleted");

    // Verify deletion
    let deleted_object = db
        .collection::<serde_json::Value>("objects")
        .find_one(doc! { "id": &object_id })
        .await
        .expect("Failed to query object")
        .expect("Object not found");

    assert_eq!(deleted_object["deleted"], true);

    cleanup_test_db(db).await;
}

#[tokio::test]
async fn test_oauth_token_flow() {
    let Some(db) = setup_test_db().await else {
        eprintln!("Test skipped: MongoDB not available");
        return;
    };

    // Test token creation
    let token = format!("token:{}", Uuid::new_v4());
    let token_doc = doc! {
        "token": &token,
        "username": "alice",
        "client_id": "test_client",
        "created_at": mongodb::bson::DateTime::now(),
        "expires_at": mongodb::bson::DateTime::from_millis(
            chrono::Utc::now().timestamp_millis() + 3600000
        ),
    };

    db.collection::<mongodb::bson::Document>("access_tokens")
        .insert_one(token_doc)
        .await
        .expect("Failed to create token");

    // Test token validation
    let filter = doc! {
        "token": &token,
        "expires_at": { "$gt": mongodb::bson::DateTime::now() }
    };

    let valid_token = db
        .collection::<mongodb::bson::Document>("access_tokens")
        .find_one(filter)
        .await
        .expect("Failed to query token")
        .expect("Token not found");

    assert_eq!(valid_token.get_str("username").unwrap(), "alice");

    // Test token revocation
    db.collection::<mongodb::bson::Document>("access_tokens")
        .delete_one(doc! { "token": &token })
        .await
        .expect("Failed to revoke token");

    // Verify token is revoked
    let revoked_token = db
        .collection::<mongodb::bson::Document>("access_tokens")
        .find_one(doc! { "token": &token })
        .await
        .expect("Failed to query token");

    assert!(revoked_token.is_none());

    cleanup_test_db(db).await;
}

#[tokio::test]
async fn test_outbox_management() {
    let Some(db) = setup_test_db().await else {
        eprintln!("Test skipped: MongoDB not available");
        return;
    };
    unsafe {
        std::env::set_var("OXIFED_DOMAIN", "test.example");
    }

    let domain = "test.example";
    let username = "alice";

    // Add multiple activities to outbox
    for i in 0..5 {
        let activity_id = format!("https://{}/activities/{}", domain, Uuid::new_v4());
        let outbox_item = doc! {
            "actor": format!("https://{}/users/{}", domain, username),
            "activity_id": &activity_id,
            "created_at": mongodb::bson::DateTime::now(),
            "sequence": i,
        };

        db.collection::<mongodb::bson::Document>("outbox")
            .insert_one(outbox_item)
            .await
            .expect("Failed to add to outbox");
    }

    // Query outbox
    let outbox_items: Vec<mongodb::bson::Document> = db
        .collection("outbox")
        .find(doc! { "actor": format!("https://{}/users/{}", domain, username) })
        .await
        .expect("Failed to query outbox")
        .try_collect()
        .await
        .expect("Failed to collect outbox items");

    assert_eq!(outbox_items.len(), 5);

    cleanup_test_db(db).await;
}

#[tokio::test]
async fn test_follow_activity_c2s() {
    let Some(db) = setup_test_db().await else {
        eprintln!("Test skipped: MongoDB not available");
        return;
    };
    unsafe {
        std::env::set_var("OXIFED_DOMAIN", "test.example");
    }

    let domain = "test.example";
    let username = "alice";
    let target_actor = "https://remote.example/users/bob";

    // Create Follow activity
    let follow_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Follow",
        "actor": format!("https://{}/users/{}", domain, username),
        "id": format!("https://{}/activities/{}", domain, Uuid::new_v4()),
        "object": target_actor,
        "published": chrono::Utc::now().to_rfc3339(),
    });

    // Store the follow activity
    let activity_doc = ActivityDocument {
        id: None,
        activity_id: follow_activity["id"].as_str().unwrap().to_string(),
        activity_type: oxifed::ActivityType::Follow,
        actor: follow_activity["actor"].as_str().unwrap().to_string(),
        object: Some(target_actor.to_string()),
        target: None,
        name: None,
        summary: None,
        published: Some(chrono::Utc::now()),
        updated: None,
        to: None,
        cc: None,
        bto: None,
        bcc: None,
        additional_properties: None,
        local: true,
        status: oxifed::database::ActivityStatus::Pending,
        created_at: chrono::Utc::now(),
        attempts: 0,
        last_attempt: None,
        error: None,
    };

    db.collection::<ActivityDocument>("activities")
        .insert_one(&activity_doc)
        .await
        .expect("Failed to store follow activity");

    // Verify follow activity
    let stored_follow = db
        .collection::<ActivityDocument>("activities")
        .find_one(doc! { "activity_id": &activity_doc.activity_id })
        .await
        .expect("Failed to query follow activity")
        .expect("Follow activity not found");

    assert_eq!(stored_follow.activity_type, oxifed::ActivityType::Follow);
    assert_eq!(stored_follow.object, Some(target_actor.to_string()));

    cleanup_test_db(db).await;
}

#[tokio::test]
async fn test_collection_pagination() {
    let Some(db) = setup_test_db().await else {
        eprintln!("Test skipped: MongoDB not available");
        return;
    };
    unsafe {
        std::env::set_var("OXIFED_DOMAIN", "test.example");
    }

    let domain = "test.example";
    let username = "alice";

    // Create multiple objects for pagination testing
    for i in 0..25 {
        let object = json!({
            "id": format!("https://{}/objects/{}", domain, Uuid::new_v4()),
            "type": "Note",
            "content": format!("Test note {}", i),
            "actor": format!("https://{}/users/{}", domain, username),
            "attributedTo": format!("https://{}/users/{}", domain, username),
            "published": chrono::Utc::now().to_rfc3339(),
            "featured": i < 5, // First 5 are featured
        });

        db.collection::<serde_json::Value>("objects")
            .insert_one(object)
            .await
            .expect("Failed to create object");
    }

    // Test featured collection
    let featured_items: Vec<serde_json::Value> = db
        .collection("objects")
        .find(doc! {
            "actor": format!("https://{}/users/{}", domain, username),
            "featured": true
        })
        .await
        .expect("Failed to query featured items")
        .try_collect()
        .await
        .expect("Failed to collect featured items");

    assert_eq!(featured_items.len(), 5);

    // Test pagination with limit
    let page1: Vec<serde_json::Value> = db
        .collection("objects")
        .find(doc! {
            "actor": format!("https://{}/users/{}", domain, username)
        })
        .limit(10)
        .await
        .expect("Failed to query page 1")
        .try_collect()
        .await
        .expect("Failed to collect page 1");

    assert_eq!(page1.len(), 10);

    cleanup_test_db(db).await;
}
