//! Message types for inter-service communication
//!
//! This module defines message structures that are shared between
//! Oxifed services for communication via message queues.

use crate::{Attachment, ImageAttachment};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Message trait that must be implemented by all message types
pub trait Message {
    /// Get the routing key for this message type
    fn routing_key(&self) -> &str;
}

/// Base enum for all message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageEnum {
    ProfileCreateMessage(ProfileCreateMessage),
    ProfileUpdateMessage(ProfileUpdateMessage),
    ProfileDeleteMessage(ProfileDeleteMessage),
    NoteCreateMessage(NoteCreateMessage),
    NoteUpdateMessage(NoteUpdateMessage),
    NoteDeleteMessage(NoteDeleteMessage),
    FollowActivityMessage(FollowActivityMessage),
    LikeActivityMessage(LikeActivityMessage),
    AnnounceActivityMessage(AnnounceActivityMessage),
}

/// Message format for profile creation requests
///
/// This message type is used when sending profile creation requests
/// to message queues, following the same structure as the oxiadm CLI arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileCreateMessage {
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

impl ProfileCreateMessage {
    /// Create a new profile creation message
    pub fn new(
        subject: String,
        summary: Option<String>,
        icon: Option<String>,
        properties: Option<Value>,
    ) -> Self {
        Self {
            subject,
            summary,
            icon,
            properties,
        }
    }
}

impl Message for ProfileCreateMessage {
    fn routing_key(&self) -> &str {
        "oxifed.profile.create"
    }
}

/// Message for updating a profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileUpdateMessage {
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<ImageAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

impl ProfileUpdateMessage {
    /// Create a new profile update message
    pub fn new(
        subject: String,
        summary: Option<String>,
        icon: Option<String>,
        properties: Option<Value>,
    ) -> Self {
        // Convert icon string to ImageAttachment if provided
        let icon_attachment = icon.map(|url| ImageAttachment {
            url,
            media_type: "image/jpeg".to_string(), // Default media type
        });

        Self {
            subject,
            summary,
            icon: icon_attachment,
            attachments: None,
            properties,
        }
    }
}

impl Message for ProfileUpdateMessage {
    fn routing_key(&self) -> &str {
        "oxifed.profile.update"
    }
}

/// Message for deleting a profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDeleteMessage {
    pub id: String,
    pub force: bool,
}

impl ProfileDeleteMessage {
    /// Create a new profile deletion message
    pub fn new(id: String, force: bool) -> Self {
        Self { id, force }
    }
}

impl Message for ProfileDeleteMessage {
    fn routing_key(&self) -> &str {
        "oxifed.profile.delete"
    }
}

/// Message for creating a note
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteCreateMessage {
    pub author: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

impl NoteCreateMessage {
    /// Create a new note creation message
    pub fn new(
        author: String,
        content: String,
        summary: Option<String>,
        mentions: Option<String>,
        tags: Option<String>,
        properties: Option<Value>,
    ) -> Self {
        Self {
            author,
            content,
            summary,
            mentions,
            tags,
            properties,
        }
    }
}

impl Message for NoteCreateMessage {
    fn routing_key(&self) -> &str {
        "oxifed.note.create"
    }
}

/// Message for updating a note
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteUpdateMessage {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

impl NoteUpdateMessage {
    /// Create a new note update message
    pub fn new(
        id: String,
        content: Option<String>,
        summary: Option<String>,
        tags: Option<String>,
        properties: Option<Value>,
    ) -> Self {
        Self {
            id,
            content,
            summary,
            tags,
            properties,
        }
    }
}

impl Message for NoteUpdateMessage {
    fn routing_key(&self) -> &str {
        "oxifed.note.update"
    }
}

/// Message for deleting a note
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDeleteMessage {
    pub id: String,
    pub force: bool,
}

impl NoteDeleteMessage {
    /// Create a new note deletion message
    pub fn new(id: String, force: bool) -> Self {
        Self { id, force }
    }
}

impl Message for NoteDeleteMessage {
    fn routing_key(&self) -> &str {
        "oxifed.note.delete"
    }
}

/// Message for creating a follow activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowActivityMessage {
    pub actor: String,
    pub object: String,
}

impl FollowActivityMessage {
    /// Create a new follow activity message
    pub fn new(actor: String, object: String) -> Self {
        Self { actor, object }
    }
}

impl Message for FollowActivityMessage {
    fn routing_key(&self) -> &str {
        "oxifed.activity.follow"
    }
}

/// Message for creating a like activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikeActivityMessage {
    pub actor: String,
    pub object: String,
}

impl LikeActivityMessage {
    /// Create a new like activity message
    pub fn new(actor: String, object: String) -> Self {
        Self { actor, object }
    }
}

impl Message for LikeActivityMessage {
    fn routing_key(&self) -> &str {
        "oxifed.activity.like"
    }
}

/// Message for creating an announce activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnounceActivityMessage {
    pub action: String,
    #[serde(rename = "type")]
    pub activity_type: String,
    pub actor: String,
    pub object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<String>,
}

impl AnnounceActivityMessage {
    /// Create a new announce activity message
    pub fn new(actor: String, object: String, to: Option<String>, cc: Option<String>) -> Self {
        Self {
            action: "announce".to_string(),
            activity_type: "Announce".to_string(),
            actor,
            object,
            to,
            cc,
        }
    }
}

impl Message for AnnounceActivityMessage {
    fn routing_key(&self) -> &str {
        "oxifed.activity.announce"
    }
}

/// Message for deleting an activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityDeleteMessage {
    pub action: String,
    pub id: String,
    pub force: bool,
}

impl ActivityDeleteMessage {
    /// Create a new activity deletion message
    pub fn new(id: String, force: bool) -> Self {
        Self {
            action: "delete".to_string(),
            id,
            force,
        }
    }
}

impl Message for ActivityDeleteMessage {
    fn routing_key(&self) -> &str {
        "oxifed.activity.delete"
    }
}
