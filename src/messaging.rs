//! Message types for inter-service communication
//!
//! This module defines message structures that are shared between
//! Oxifed services for communication via message queues.

use crate::{Attachment, ImageAttachment};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Base trait for all message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
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
}

/// Message for deleting a profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDeleteMessage {
    pub action: String,
    pub id: String,
    pub force: bool,
}

/// Message for creating a note
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteCreateMessage {
    pub action: String,
    pub author: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mentions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

/// Message for updating a note
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteUpdateMessage {
    pub action: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

/// Message for deleting a note
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDeleteMessage {
    pub action: String,
    pub id: String,
    pub force: bool,
}

/// Message for creating a follow activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowActivityMessage {
    pub action: String,
    #[serde(rename = "type")]
    pub activity_type: String,
    pub actor: String,
    pub object: String,
}

/// Message for creating a like activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LikeActivityMessage {
    pub action: String,
    #[serde(rename = "type")]
    pub activity_type: String,
    pub actor: String,
    pub object: String,
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
