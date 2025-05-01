//! Message types for inter-service communication
//!
//! This module defines message structures that are shared between
//! Oxifed services for communication via message queues.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Base trait for all message types
pub trait Message: Serialize {
    /// Returns the routing key for this message
    fn routing_key(&self) -> &str;
}

/// Message format for profile creation requests
///
/// This message type is used when sending profile creation requests
/// to message queues, following the same structure as the oxiadm CLI arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileCreateMessage {
    pub action: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

impl ProfileCreateMessage {
    pub fn new(
        subject: String,
        name: Option<String>,
        summary: Option<String>,
        icon: Option<String>,
        properties: Option<Value>,
    ) -> Self {
        Self {
            action: "create_person".to_string(),
            subject,
            name,
            summary,
            icon,
            properties,
        }
    }
}

impl Message for ProfileCreateMessage {
    fn routing_key(&self) -> &str {
        "person.create"
    }
}

/// Message for updating a profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileUpdateMessage {
    pub action: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

impl ProfileUpdateMessage {
    pub fn new(
        id: String,
        name: Option<String>,
        summary: Option<String>,
        icon: Option<String>,
        properties: Option<Value>,
    ) -> Self {
        Self {
            action: "update_person".to_string(),
            id,
            name,
            summary,
            icon,
            properties,
        }
    }
}

impl Message for ProfileUpdateMessage {
    fn routing_key(&self) -> &str {
        "person.update"
    }
}

/// Message for deleting a profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDeleteMessage {
    pub action: String,
    pub id: String,
    pub force: bool,
}

impl ProfileDeleteMessage {
    pub fn new(id: String, force: bool) -> Self {
        Self {
            action: "delete_person".to_string(),
            id,
            force,
        }
    }
}

impl Message for ProfileDeleteMessage {
    fn routing_key(&self) -> &str {
        "person.delete"
    }
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

impl NoteCreateMessage {
    pub fn new(
        author: String,
        content: String,
        name: Option<String>,
        summary: Option<String>,
        mentions: Option<String>,
        tags: Option<String>,
        properties: Option<Value>,
    ) -> Self {
        Self {
            action: "create_note".to_string(),
            author,
            content,
            name,
            summary,
            mentions,
            tags,
            properties,
        }
    }
}

impl Message for NoteCreateMessage {
    fn routing_key(&self) -> &str {
        "note.create"
    }
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

impl NoteUpdateMessage {
    pub fn new(
        id: String,
        content: Option<String>,
        name: Option<String>,
        summary: Option<String>,
        tags: Option<String>,
        properties: Option<Value>,
    ) -> Self {
        Self {
            action: "update_note".to_string(),
            id,
            content,
            name,
            summary,
            tags,
            properties,
        }
    }
}

impl Message for NoteUpdateMessage {
    fn routing_key(&self) -> &str {
        "note.update"
    }
}

/// Message for deleting a note
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDeleteMessage {
    pub action: String,
    pub id: String,
    pub force: bool,
}

impl NoteDeleteMessage {
    pub fn new(id: String, force: bool) -> Self {
        Self {
            action: "delete_note".to_string(),
            id,
            force,
        }
    }
}

impl Message for NoteDeleteMessage {
    fn routing_key(&self) -> &str {
        "note.delete"
    }
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

impl FollowActivityMessage {
    pub fn new(actor: String, object: String) -> Self {
        Self {
            action: "create_activity".to_string(),
            activity_type: "Follow".to_string(),
            actor,
            object,
        }
    }
}

impl Message for FollowActivityMessage {
    fn routing_key(&self) -> &str {
        "activity.follow"
    }
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

impl LikeActivityMessage {
    pub fn new(actor: String, object: String) -> Self {
        Self {
            action: "create_activity".to_string(),
            activity_type: "Like".to_string(),
            actor,
            object,
        }
    }
}

impl Message for LikeActivityMessage {
    fn routing_key(&self) -> &str {
        "activity.like"
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
    pub fn new(
        actor: String,
        object: String,
        to: Option<String>,
        cc: Option<String>,
    ) -> Self {
        Self {
            action: "create_activity".to_string(),
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
        "activity.announce"
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
    pub fn new(id: String, force: bool) -> Self {
        Self {
            action: "delete_activity".to_string(),
            id,
            force,
        }
    }
}

impl Message for ActivityDeleteMessage {
    fn routing_key(&self) -> &str {
        "activity.delete"
    }
}

/// Legacy message format for profile creation requests
///
/// This message type is kept for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyProfileCreateMessage {
    /// Name of the profile (used for display only)
    pub name: String,
    
    /// Subject of the profile (e.g. user@example.com or full acct:user@example.com)
    pub subject: String,
    
    /// Optional aliases for the profile (comma separated)
    pub aliases: Option<String>,
    
    /// Optional links to add to the profile (format: rel,href[,title][;rel2,href2,...])
    pub links: Option<String>,
}

/// Legacy message format for profile edit requests
///
/// This message type is kept for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyProfileEditMessage {
    /// Original subject of the profile to edit (with or without acct: prefix)
    pub subject: String,
    
    /// New subject for the profile (with or without acct: prefix)
    pub new_subject: Option<String>,
    
    /// New aliases for the profile (comma separated)
    pub aliases: Option<String>,
    
    /// Add links to the profile (format: rel,href[,title][;rel2,href2,...])
    pub links: Option<String>,
    
    /// Remove all aliases
    pub clear_aliases: bool,
    
    /// Remove all links
    pub clear_links: bool,
}
