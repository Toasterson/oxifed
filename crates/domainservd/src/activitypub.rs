//! ActivityPub endpoints for domainservd
//!
//! Implements the ActivityPub server-to-server protocol endpoints including
//! actor profiles, inboxes, outboxes, and collections.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use oxifed::{
    ActivityType, ObjectType,
    database::{
        ActivityDocument, ActivityStatus, ActorDocument, ActorStatus, DatabaseManager,
        FollowDocument, FollowStatus, ObjectDocument, VisibilityLevel,
    },
    pki::PkiManager,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Application state for ActivityPub handlers
#[derive(Clone)]
pub struct ActivityPubState {
    pub db: Arc<DatabaseManager>,
    pub pki: Arc<PkiManager>,
    pub mq_pool: deadpool_lapin::Pool,
    pub domain: String,
}

/// Query parameters for collections
#[derive(Debug, Deserialize)]
pub struct CollectionQuery {
    page: Option<bool>,
    min_id: Option<String>,
    max_id: Option<String>,
    since_id: Option<String>,
    limit: Option<u32>,
}

/// ActivityPub collection response
#[derive(Debug, Serialize)]
pub struct ActivityPubCollection {
    #[serde(rename = "@context")]
    context: Vec<String>,
    #[serde(rename = "type")]
    collection_type: String,
    id: String,
    total_items: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    items: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ordered_items: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    first: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prev: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    part_of: Option<String>,
}

/// Create ActivityPub router
pub fn activitypub_router(state: ActivityPubState) -> Router<ActivityPubState> {
    Router::new()
        // Actor endpoints
        .route("/users/:username", get(get_actor))
        .route("/users/:username/inbox", post(post_inbox))
        .route("/users/:username/outbox", get(get_outbox).post(post_outbox))
        .route("/users/:username/followers", get(get_followers))
        .route("/users/:username/following", get(get_following))
        .route("/users/:username/liked", get(get_liked))
        .route("/users/:username/featured", get(get_featured))
        // Object endpoints
        .route("/objects/:id", get(get_object))
        .route("/activities/:id", get(get_activity))
        // Shared inbox
        .route("/inbox", post(post_shared_inbox))
        // Node info
        .route("/nodeinfo/2.0", get(get_nodeinfo))
        .with_state(state)
}

/// Get actor profile
async fn get_actor(
    Path(username): Path<String>,
    State(state): State<ActivityPubState>,
    _headers: HeaderMap,
) -> Result<Response, StatusCode> {
    debug!("Getting actor profile for username: {}", username);

    // Find actor in database
    let actor_doc = match state
        .db
        .find_actor_by_username(&username, &state.domain)
        .await
    {
        Ok(Some(actor)) => actor,
        Ok(None) => {
            warn!("Actor not found: {}@{}", username, state.domain);
            return Err(StatusCode::NOT_FOUND);
        }
        Err(e) => {
            error!("Database error finding actor: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Check if actor is active
    if actor_doc.status != ActorStatus::Active {
        warn!("Actor not active: {}@{}", username, state.domain);
        return Err(StatusCode::GONE);
    }

    // Convert to ActivityPub format
    let actor_json = json!({
        "@context": [
            "https://www.w3.org/ns/activitystreams",
            "https://w3id.org/security/v1",
            {
                "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
                "toot": "http://joinmastodon.org/ns#",
                "featured": {
                    "@id": "toot:featured",
                    "@type": "@id"
                },
                "PropertyValue": "schema:PropertyValue",
                "value": "schema:value"
            }
        ],
        "type": "Person",
        "id": actor_doc.actor_id,
        "name": actor_doc.name,
        "preferredUsername": actor_doc.preferred_username,
        "summary": actor_doc.summary,
        "icon": actor_doc.icon.map(|url| json!({
            "type": "Image",
            "url": url
        })),
        "image": actor_doc.image.map(|url| json!({
            "type": "Image",
            "url": url
        })),
        "inbox": actor_doc.inbox,
        "outbox": actor_doc.outbox,
        "following": actor_doc.following,
        "followers": actor_doc.followers,
        "liked": actor_doc.liked,
        "featured": actor_doc.featured,
        "endpoints": actor_doc.endpoints,
        "attachment": actor_doc.attachment,
        "publicKey": actor_doc.public_key.map(|pk| json!({
            "id": pk.id,
            "owner": pk.owner,
            "publicKeyPem": pk.public_key_pem
        })),
        "published": actor_doc.created_at.to_rfc3339(),
        "manuallyApprovesFollowers": false
    });

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/activity+json")],
        Json(actor_json),
    )
        .into_response())
}

/// Handle incoming activities to user inbox
async fn post_inbox(
    Path(username): Path<String>,
    State(state): State<ActivityPubState>,
    headers: HeaderMap,
    Json(activity): Json<Value>,
) -> Result<Response, StatusCode> {
    info!("Received activity for user inbox: {}", username);
    debug!(
        "Activity payload: {}",
        serde_json::to_string_pretty(&activity).unwrap_or_default()
    );

    // Verify HTTP signature
    if let Err(e) = verify_http_signature(&headers, &state).await {
        warn!("HTTP signature verification failed: {}", e);
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Verify actor exists and is active
    let actor_doc = match state
        .db
        .find_actor_by_username(&username, &state.domain)
        .await
    {
        Ok(Some(actor)) => actor,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if actor_doc.status != ActorStatus::Active {
        return Err(StatusCode::GONE);
    }

    // Process the activity
    match process_incoming_activity(&activity, &actor_doc, &state).await {
        Ok(_) => Ok(StatusCode::ACCEPTED.into_response()),
        Err(e) => {
            error!("Failed to process incoming activity: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// Handle shared inbox for server-level activities
async fn post_shared_inbox(
    State(state): State<ActivityPubState>,
    headers: HeaderMap,
    Json(activity): Json<Value>,
) -> Result<Response, StatusCode> {
    info!("Received activity for shared inbox");
    debug!(
        "Activity payload: {}",
        serde_json::to_string_pretty(&activity).unwrap_or_default()
    );

    // Verify HTTP signature
    if let Err(e) = verify_http_signature(&headers, &state).await {
        warn!("HTTP signature verification failed: {}", e);
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Process the activity
    match process_shared_inbox_activity(&activity, &state).await {
        Ok(_) => Ok(StatusCode::ACCEPTED.into_response()),
        Err(e) => {
            error!("Failed to process shared inbox activity: {}", e);
            Err(StatusCode::BAD_REQUEST)
        }
    }
}

/// Get actor's outbox
async fn get_outbox(
    Path(username): Path<String>,
    Query(params): Query<CollectionQuery>,
    State(state): State<ActivityPubState>,
    _headers: HeaderMap,
) -> Result<Response, StatusCode> {
    debug!("Getting outbox for user: {}", username);

    // Find actor
    let actor_doc = match state
        .db
        .find_actor_by_username(&username, &state.domain)
        .await
    {
        Ok(Some(actor)) => actor,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if actor_doc.status != ActorStatus::Active {
        return Err(StatusCode::GONE);
    }

    let limit = params.limit.unwrap_or(20).min(40) as i64;
    let offset = 0; // TODO: Implement proper pagination

    // Get actor's objects
    let objects = match state
        .db
        .get_actor_outbox(&actor_doc.actor_id, limit, offset)
        .await
    {
        Ok(objects) => objects,
        Err(e) => {
            error!("Failed to get actor outbox: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Convert to ActivityPub format
    let items: Vec<Value> = objects
        .into_iter()
        .map(|obj| {
            json!({
                "type": "Create",
                "id": format!("{}/activities/{}", actor_doc.actor_id, Uuid::new_v4()),
                "actor": actor_doc.actor_id,
                "published": obj.published.unwrap_or(obj.created_at).to_rfc3339(),
                "object": {
                    "type": format!("{:?}", obj.object_type),
                    "id": obj.object_id,
                    "attributedTo": obj.attributed_to,
                    "content": obj.content,
                    "summary": obj.summary,
                    "published": obj.published.unwrap_or(obj.created_at).to_rfc3339(),
                    "to": obj.to,
                    "cc": obj.cc
                }
            })
        })
        .collect();

    let collection = ActivityPubCollection {
        context: vec!["https://www.w3.org/ns/activitystreams".to_string()],
        collection_type: "OrderedCollection".to_string(),
        id: actor_doc.outbox,
        total_items: Some(actor_doc.statuses_count as u64),
        ordered_items: Some(items),
        items: None,
        first: None,
        last: None,
        next: None,
        prev: None,
        part_of: None,
    };

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/activity+json")],
        Json(collection),
    )
        .into_response())
}

/// Post to actor's outbox (C2S)
async fn post_outbox(
    Path(_username): Path<String>,
    State(_state): State<ActivityPubState>,
    _headers: HeaderMap,
    Json(_activity): Json<Value>,
) -> Result<Response, StatusCode> {
    info!("Posting to outbox for user: {}", _username);

    // TODO: Implement authentication for C2S
    // For now, reject all C2S posts
    Err(StatusCode::FORBIDDEN)
}

/// Get actor's followers
async fn get_followers(
    Path(username): Path<String>,
    State(state): State<ActivityPubState>,
    _headers: HeaderMap,
) -> Result<Response, StatusCode> {
    debug!("Getting followers for user: {}", username);

    let actor_doc = match state
        .db
        .find_actor_by_username(&username, &state.domain)
        .await
    {
        Ok(Some(actor)) => actor,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if actor_doc.status != ActorStatus::Active {
        return Err(StatusCode::GONE);
    }

    let followers = match state.db.get_actor_followers(&actor_doc.actor_id).await {
        Ok(followers) => followers,
        Err(e) => {
            error!("Failed to get followers: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let collection = ActivityPubCollection {
        context: vec!["https://www.w3.org/ns/activitystreams".to_string()],
        collection_type: "OrderedCollection".to_string(),
        id: actor_doc.followers,
        total_items: Some(followers.len() as u64),
        ordered_items: Some(followers.into_iter().map(|f| json!(f)).collect()),
        items: None,
        first: None,
        last: None,
        next: None,
        prev: None,
        part_of: None,
    };

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/activity+json")],
        Json(collection),
    )
        .into_response())
}

/// Get actor's following
async fn get_following(
    Path(username): Path<String>,
    State(state): State<ActivityPubState>,
    _headers: HeaderMap,
) -> Result<Response, StatusCode> {
    debug!("Getting following for user: {}", username);

    let actor_doc = match state
        .db
        .find_actor_by_username(&username, &state.domain)
        .await
    {
        Ok(Some(actor)) => actor,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if actor_doc.status != ActorStatus::Active {
        return Err(StatusCode::GONE);
    }

    let following = match state.db.get_actor_following(&actor_doc.actor_id).await {
        Ok(following) => following,
        Err(e) => {
            error!("Failed to get following: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let collection = ActivityPubCollection {
        context: vec!["https://www.w3.org/ns/activitystreams".to_string()],
        collection_type: "OrderedCollection".to_string(),
        id: actor_doc.following,
        total_items: Some(following.len() as u64),
        ordered_items: Some(following.into_iter().map(|f| json!(f)).collect()),
        items: None,
        first: None,
        last: None,
        next: None,
        prev: None,
        part_of: None,
    };

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/activity+json")],
        Json(collection),
    )
        .into_response())
}

/// Get actor's liked collection
async fn get_liked(
    Path(_username): Path<String>,
    State(_state): State<ActivityPubState>,
) -> Result<Response, StatusCode> {
    // Liked collections are typically private
    Err(StatusCode::FORBIDDEN)
}

/// Get actor's featured collection
async fn get_featured(
    Path(username): Path<String>,
    State(state): State<ActivityPubState>,
) -> Result<Response, StatusCode> {
    debug!("Getting featured posts for user: {}", username);

    let actor_doc = match state
        .db
        .find_actor_by_username(&username, &state.domain)
        .await
    {
        Ok(Some(actor)) => actor,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if actor_doc.status != ActorStatus::Active {
        return Err(StatusCode::GONE);
    }

    // For now, return empty collection
    let collection = ActivityPubCollection {
        context: vec!["https://www.w3.org/ns/activitystreams".to_string()],
        collection_type: "OrderedCollection".to_string(),
        id: actor_doc
            .featured
            .unwrap_or_else(|| format!("{}/featured", actor_doc.actor_id)),
        total_items: Some(0),
        ordered_items: Some(vec![]),
        items: None,
        first: None,
        last: None,
        next: None,
        prev: None,
        part_of: None,
    };

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/activity+json")],
        Json(collection),
    )
        .into_response())
}

/// Get individual object
async fn get_object(
    Path(id): Path<String>,
    State(state): State<ActivityPubState>,
) -> Result<Response, StatusCode> {
    debug!("Getting object: {}", id);

    let object_id = format!("https://{}/objects/{}", state.domain, id);

    let object_doc = match state.db.find_object_by_id(&object_id).await {
        Ok(Some(obj)) => obj,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get object: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let object_json = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": format!("{:?}", object_doc.object_type),
        "id": object_doc.object_id,
        "attributedTo": object_doc.attributed_to,
        "content": object_doc.content,
        "summary": object_doc.summary,
        "published": object_doc.published.unwrap_or(object_doc.created_at).to_rfc3339(),
        "to": object_doc.to,
        "cc": object_doc.cc,
        "inReplyTo": object_doc.in_reply_to,
        "conversation": object_doc.conversation,
        "sensitive": object_doc.sensitive,
        "tag": object_doc.tag,
        "attachment": object_doc.attachment
    });

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/activity+json")],
        Json(object_json),
    )
        .into_response())
}

/// Get individual activity
async fn get_activity(
    Path(id): Path<String>,
    State(state): State<ActivityPubState>,
) -> Result<Response, StatusCode> {
    debug!("Getting activity: {}", id);

    let activity_id = format!("https://{}/activities/{}", state.domain, id);

    let activity_doc = match state.db.find_activity_by_id(&activity_id).await {
        Ok(Some(activity)) => activity,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get activity: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let activity_json = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": format!("{:?}", activity_doc.activity_type),
        "id": activity_doc.activity_id,
        "actor": activity_doc.actor,
        "object": activity_doc.object,
        "target": activity_doc.target,
        "published": activity_doc.published.unwrap_or(activity_doc.created_at).to_rfc3339(),
        "to": activity_doc.to,
        "cc": activity_doc.cc
    });

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/activity+json")],
        Json(activity_json),
    )
        .into_response())
}

/// Get node info
async fn get_nodeinfo(State(state): State<ActivityPubState>) -> Result<Response, StatusCode> {
    let nodeinfo = json!({
        "version": "2.0",
        "software": {
            "name": "oxifed",
            "version": "0.1.0"
        },
        "protocols": ["activitypub"],
        "usage": {
            "users": {
                "total": 0,
                "activeMonth": 0,
                "activeHalfyear": 0
            },
            "localPosts": 0
        },
        "openRegistrations": false,
        "metadata": {
            "nodeName": state.domain,
            "nodeDescription": "Oxifed ActivityPub server"
        }
    });

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/json")],
        Json(nodeinfo),
    )
        .into_response())
}

/// Verify HTTP signature
async fn verify_http_signature(
    _headers: &HeaderMap,
    _state: &ActivityPubState,
) -> Result<(), String> {
    // TODO: Implement proper HTTP signature verification using PKI
    debug!("HTTP signature verification - placeholder implementation");
    Ok(())
}

/// Process incoming activity for a specific user
async fn process_incoming_activity(
    activity: &Value,
    actor: &ActorDocument,
    state: &ActivityPubState,
) -> Result<(), String> {
    let activity_type = activity
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or("Missing activity type")?;

    info!(
        "Processing {} activity for {}",
        activity_type, actor.actor_id
    );

    match activity_type {
        "Follow" => handle_follow_activity(activity, actor, state).await,
        "Undo" => handle_undo_activity(activity, actor, state).await,
        "Create" => handle_create_activity(activity, actor, state).await,
        "Update" => handle_update_activity(activity, actor, state).await,
        "Delete" => handle_delete_activity(activity, actor, state).await,
        "Like" => handle_like_activity(activity, actor, state).await,
        "Announce" => handle_announce_activity(activity, actor, state).await,
        _ => {
            warn!("Unhandled activity type: {}", activity_type);
            Ok(())
        }
    }
}

/// Process shared inbox activity
async fn process_shared_inbox_activity(
    activity: &Value,
    state: &ActivityPubState,
) -> Result<(), String> {
    let activity_type = activity
        .get("type")
        .and_then(|t| t.as_str())
        .ok_or("Missing activity type")?;

    info!("Processing {} activity in shared inbox", activity_type);

    // Determine target actors and route accordingly
    if let Some(to) = activity.get("to") {
        // Route to specific actors if needed
        debug!("Activity addressed to: {:?}", to);
    }

    // For now, just store the activity
    store_activity(activity, state).await
}

/// Handle Follow activity
async fn handle_follow_activity(
    activity: &Value,
    target_actor: &ActorDocument,
    state: &ActivityPubState,
) -> Result<(), String> {
    let follower = activity
        .get("actor")
        .and_then(|a| a.as_str())
        .ok_or("Missing follower actor")?;

    info!(
        "Processing follow from {} to {}",
        follower, target_actor.actor_id
    );

    // Create follow relationship
    let follow_doc = FollowDocument {
        id: None,
        follower: follower.to_string(),
        following: target_actor.actor_id.clone(),
        status: FollowStatus::Pending,
        activity_id: activity
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or("unknown")
            .to_string(),
        accept_activity_id: None,
        created_at: Utc::now(),
        responded_at: None,
    };

    state
        .db
        .insert_follow(follow_doc)
        .await
        .map_err(|e| format!("Failed to store follow: {}", e))?;

    // Auto-accept for now (TODO: Check actor preferences)
    let accept_activity = json!({
        "@context": "https://www.w3.org/ns/activitystreams",
        "type": "Accept",
        "id": format!("{}/activities/{}", target_actor.actor_id, Uuid::new_v4()),
        "actor": target_actor.actor_id,
        "object": activity,
        "published": Utc::now().to_rfc3339()
    });

    // Send accept back (via message queue)
    publish_activity_message(&accept_activity, state).await?;

    // Update follow status
    state
        .db
        .update_follow_status(follower, &target_actor.actor_id, FollowStatus::Accepted)
        .await
        .map_err(|e| format!("Failed to update follow status: {}", e))?;

    Ok(())
}

/// Handle Undo activity
async fn handle_undo_activity(
    activity: &Value,
    actor: &ActorDocument,
    state: &ActivityPubState,
) -> Result<(), String> {
    let object = activity.get("object").ok_or("Missing undo object")?;

    if let Some(object_type) = object.get("type").and_then(|t| t.as_str()) {
        match object_type {
            "Follow" => {
                let following = object
                    .get("object")
                    .and_then(|o| o.as_str())
                    .ok_or("Missing follow target")?;

                info!(
                    "Processing unfollow from {} to {}",
                    actor.actor_id, following
                );

                state
                    .db
                    .update_follow_status(&actor.actor_id, following, FollowStatus::Cancelled)
                    .await
                    .map_err(|e| format!("Failed to update follow status: {}", e))?;
            }
            _ => {
                warn!("Unhandled undo object type: {}", object_type);
            }
        }
    }

    Ok(())
}

/// Handle Create activity
async fn handle_create_activity(
    activity: &Value,
    actor: &ActorDocument,
    state: &ActivityPubState,
) -> Result<(), String> {
    let object = activity.get("object").ok_or("Missing create object")?;

    if let Some(object_type) = object.get("type").and_then(|t| t.as_str()) {
        match object_type {
            "Note" => {
                info!("Processing note creation from {}", actor.actor_id);
                store_note_object(object, state).await?;
            }
            "Article" => {
                info!("Processing article creation from {}", actor.actor_id);
                store_article_object(object, state).await?;
            }
            _ => {
                warn!("Unhandled create object type: {}", object_type);
            }
        }
    }

    store_activity(activity, state).await
}

/// Handle Update activity
async fn handle_update_activity(
    activity: &Value,
    actor: &ActorDocument,
    state: &ActivityPubState,
) -> Result<(), String> {
    info!("Processing update activity from {}", actor.actor_id);
    store_activity(activity, state).await
}

/// Handle Delete activity
async fn handle_delete_activity(
    activity: &Value,
    actor: &ActorDocument,
    state: &ActivityPubState,
) -> Result<(), String> {
    info!("Processing delete activity from {}", actor.actor_id);
    store_activity(activity, state).await
}

/// Handle Like activity
async fn handle_like_activity(
    activity: &Value,
    actor: &ActorDocument,
    state: &ActivityPubState,
) -> Result<(), String> {
    info!("Processing like activity from {}", actor.actor_id);
    store_activity(activity, state).await
}

/// Handle Announce activity
async fn handle_announce_activity(
    activity: &Value,
    actor: &ActorDocument,
    state: &ActivityPubState,
) -> Result<(), String> {
    info!("Processing announce activity from {}", actor.actor_id);
    store_activity(activity, state).await
}

/// Store activity in database
async fn store_activity(activity: &Value, state: &ActivityPubState) -> Result<(), String> {
    let activity_doc = ActivityDocument {
        id: None,
        activity_id: activity
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or(&format!("unknown-{}", Uuid::new_v4()))
            .to_string(),
        activity_type: parse_activity_type(activity.get("type")),
        actor: activity
            .get("actor")
            .and_then(|a| a.as_str())
            .unwrap_or("unknown")
            .to_string(),
        object: activity
            .get("object")
            .and_then(|o| o.as_str())
            .map(|s| s.to_string()),
        target: activity
            .get("target")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        name: activity
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string()),
        summary: activity
            .get("summary")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string()),
        published: activity
            .get("published")
            .and_then(|p| p.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc)),
        updated: None,
        to: extract_string_array(activity.get("to")),
        cc: extract_string_array(activity.get("cc")),
        bto: extract_string_array(activity.get("bto")),
        bcc: extract_string_array(activity.get("bcc")),
        additional_properties: None,
        local: false,
        status: ActivityStatus::Completed,
        created_at: Utc::now(),
        attempts: 0,
        last_attempt: None,
        error: None,
    };

    state
        .db
        .insert_activity(activity_doc)
        .await
        .map_err(|e| format!("Failed to store activity: {}", e))?;

    Ok(())
}

/// Store note object in database
async fn store_note_object(object: &Value, state: &ActivityPubState) -> Result<(), String> {
    let object_doc = ObjectDocument {
        id: None,
        object_id: object
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or(&format!("unknown-{}", Uuid::new_v4()))
            .to_string(),
        object_type: ObjectType::Note,
        attributed_to: object
            .get("attributedTo")
            .and_then(|a| a.as_str())
            .unwrap_or("unknown")
            .to_string(),
        content: object
            .get("content")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string()),
        summary: object
            .get("summary")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string()),
        name: object
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string()),
        media_type: Some("text/html".to_string()),
        url: object
            .get("url")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string()),
        published: object
            .get("published")
            .and_then(|p| p.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc)),
        updated: object
            .get("updated")
            .and_then(|u| u.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc)),
        to: extract_string_array(object.get("to")),
        cc: extract_string_array(object.get("cc")),
        bto: extract_string_array(object.get("bto")),
        bcc: extract_string_array(object.get("bcc")),
        audience: extract_string_array(object.get("audience")),
        in_reply_to: object
            .get("inReplyTo")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string()),
        conversation: object
            .get("conversation")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string()),
        tag: None,        // TODO: Parse tags
        attachment: None, // TODO: Parse attachments
        language: object
            .get("language")
            .and_then(|l| l.as_str())
            .map(|s| s.to_string()),
        sensitive: object.get("sensitive").and_then(|s| s.as_bool()),
        additional_properties: None,
        local: false,
        visibility: VisibilityLevel::Public, // TODO: Determine visibility
        created_at: Utc::now(),
        reply_count: 0,
        like_count: 0,
        announce_count: 0,
    };

    state
        .db
        .insert_object(object_doc)
        .await
        .map_err(|e| format!("Failed to store note object: {}", e))?;

    Ok(())
}

/// Store article object in database
async fn store_article_object(object: &Value, state: &ActivityPubState) -> Result<(), String> {
    let object_doc = ObjectDocument {
        id: None,
        object_id: object
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or(&format!("unknown-{}", Uuid::new_v4()))
            .to_string(),
        object_type: ObjectType::Article,
        attributed_to: object
            .get("attributedTo")
            .and_then(|a| a.as_str())
            .unwrap_or("unknown")
            .to_string(),
        content: object
            .get("content")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string()),
        summary: object
            .get("summary")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string()),
        name: object
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string()),
        media_type: Some("text/html".to_string()),
        url: object
            .get("url")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string()),
        published: object
            .get("published")
            .and_then(|p| p.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc)),
        updated: object
            .get("updated")
            .and_then(|u| u.as_str())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc)),
        to: extract_string_array(object.get("to")),
        cc: extract_string_array(object.get("cc")),
        bto: extract_string_array(object.get("bto")),
        bcc: extract_string_array(object.get("bcc")),
        audience: extract_string_array(object.get("audience")),
        in_reply_to: object
            .get("inReplyTo")
            .and_then(|r| r.as_str())
            .map(|s| s.to_string()),
        conversation: object
            .get("conversation")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string()),
        tag: None,        // TODO: Parse tags
        attachment: None, // TODO: Parse attachments
        language: object
            .get("language")
            .and_then(|l| l.as_str())
            .map(|s| s.to_string()),
        sensitive: object.get("sensitive").and_then(|s| s.as_bool()),
        additional_properties: None,
        local: false,
        visibility: VisibilityLevel::Public, // TODO: Determine visibility
        created_at: Utc::now(),
        reply_count: 0,
        like_count: 0,
        announce_count: 0,
    };

    state
        .db
        .insert_object(object_doc)
        .await
        .map_err(|e| format!("Failed to store article object: {}", e))?;

    Ok(())
}

/// Publish activity to message queue for delivery
async fn publish_activity_message(
    activity: &Value,
    _state: &ActivityPubState,
) -> Result<(), String> {
    // TODO: Implement message queue publishing
    debug!(
        "Publishing activity to message queue: {}",
        activity.get("type").unwrap_or(&json!("Unknown"))
    );
    Ok(())
}

/// Parse ActivityPub activity type
fn parse_activity_type(type_value: Option<&Value>) -> ActivityType {
    match type_value.and_then(|t| t.as_str()) {
        Some("Accept") => ActivityType::Accept,
        Some("Add") => ActivityType::Add,
        Some("Announce") => ActivityType::Announce,
        Some("Arrive") => ActivityType::Arrive,
        Some("Block") => ActivityType::Block,
        Some("Create") => ActivityType::Create,
        Some("Delete") => ActivityType::Delete,
        Some("Dislike") => ActivityType::Dislike,
        Some("Flag") => ActivityType::Flag,
        Some("Follow") => ActivityType::Follow,
        Some("Ignore") => ActivityType::Ignore,
        Some("Invite") => ActivityType::Invite,
        Some("Join") => ActivityType::Join,
        Some("Leave") => ActivityType::Leave,
        Some("Like") => ActivityType::Like,
        Some("Listen") => ActivityType::Listen,
        Some("Move") => ActivityType::Move,
        Some("Offer") => ActivityType::Offer,
        Some("Question") => ActivityType::Question,
        Some("Reject") => ActivityType::Reject,
        Some("Read") => ActivityType::Read,
        Some("Remove") => ActivityType::Remove,
        Some("TentativeReject") => ActivityType::TentativeReject,
        Some("TentativeAccept") => ActivityType::TentativeAccept,
        Some("Travel") => ActivityType::Travel,
        Some("Undo") => ActivityType::Undo,
        Some("Update") => ActivityType::Update,
        Some("View") => ActivityType::View,
        _ => ActivityType::Other,
    }
}

/// Extract string array from JSON value
fn extract_string_array(value: Option<&Value>) -> Option<Vec<String>> {
    match value {
        Some(Value::Array(arr)) => {
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect();
            if strings.is_empty() {
                None
            } else {
                Some(strings)
            }
        }
        Some(Value::String(s)) => Some(vec![s.clone()]),
        _ => None,
    }
}
