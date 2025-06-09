//! Message types for inter-service communication
//!
//! This module defines message structures that are shared between
//! Oxifed services for communication via message queues.

use crate::{Attachment, ImageAttachment};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Constants for RabbitMQ Exchange names
pub const EXCHANGE_INTERNAL_PUBLISH: &str = "oxifed.internal.publish";
pub const EXCHANGE_ACTIVITYPUB_PUBLISH: &str = "oxifed.activitypub.publish";
pub const EXCHANGE_INCOMING_PROCESS: &str = "oxifed.incoming.process";
pub const EXCHANGE_RPC_REQUEST: &str = "oxifed.rpc.request";
pub const EXCHANGE_RPC_RESPONSE: &str = "oxifed.rpc.response";

/// Constants for RabbitMQ Queue names
pub const QUEUE_RPC_DOMAIN: &str = "oxifed.rpc.domain";

/// Message trait that must be implemented by all message types
pub trait Message {
    fn to_message(&self) -> MessageEnum;
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
    AcceptActivityMessage(AcceptActivityMessage),
    RejectActivityMessage(RejectActivityMessage),
    DomainCreateMessage(DomainCreateMessage),
    DomainUpdateMessage(DomainUpdateMessage),
    DomainDeleteMessage(DomainDeleteMessage),
    DomainRpcRequest(DomainRpcRequest),
    DomainRpcResponse(DomainRpcResponse),
    IncomingObjectMessage(IncomingObjectMessage),
    IncomingActivityMessage(IncomingActivityMessage),
    KeyGenerateMessage(KeyGenerateMessage),
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
    fn to_message(&self) -> MessageEnum {
        MessageEnum::ProfileCreateMessage(self.clone())
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
    fn to_message(&self) -> MessageEnum {
        MessageEnum::ProfileUpdateMessage(self.clone())
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
    fn to_message(&self) -> MessageEnum {
        MessageEnum::ProfileDeleteMessage(self.clone())
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
    fn to_message(&self) -> MessageEnum {
        MessageEnum::NoteCreateMessage(self.clone())
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
    fn to_message(&self) -> MessageEnum {
        MessageEnum::NoteUpdateMessage(self.clone())
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
    fn to_message(&self) -> MessageEnum {
        MessageEnum::NoteDeleteMessage(self.clone())
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
    fn to_message(&self) -> MessageEnum {
        MessageEnum::FollowActivityMessage(self.clone())
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
    fn to_message(&self) -> MessageEnum {
        MessageEnum::LikeActivityMessage(self.clone())
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
    fn to_message(&self) -> MessageEnum {
        MessageEnum::AnnounceActivityMessage(self.clone())
    }
}

/// Message for creating an accept activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptActivityMessage {
    pub actor: String,
    pub object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<String>,
}

impl AcceptActivityMessage {
    /// Create a new accept activity message
    pub fn new(actor: String, object: String, to: Option<String>, cc: Option<String>) -> Self {
        Self {
            actor,
            object,
            to,
            cc,
        }
    }
}

impl Message for AcceptActivityMessage {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::AcceptActivityMessage(self.clone())
    }
}

/// Message for creating a reject activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectActivityMessage {
    pub actor: String,
    pub object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<String>,
}

impl RejectActivityMessage {
    /// Create a new reject activity message
    pub fn new(actor: String, object: String, to: Option<String>, cc: Option<String>) -> Self {
        Self {
            actor,
            object,
            to,
            cc,
        }
    }
}

impl Message for RejectActivityMessage {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::RejectActivityMessage(self.clone())
    }
}

/// Message for creating a domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainCreateMessage {
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized_fetch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_note_length: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_file_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

impl DomainCreateMessage {
    /// Create a new domain creation message
    pub fn new(
        domain: String,
        name: Option<String>,
        description: Option<String>,
        contact_email: Option<String>,
        rules: Option<Vec<String>>,
        registration_mode: Option<String>,
        authorized_fetch: Option<bool>,
        max_note_length: Option<i32>,
        max_file_size: Option<i64>,
        allowed_file_types: Option<Vec<String>>,
        properties: Option<Value>,
    ) -> Self {
        Self {
            domain,
            name,
            description,
            contact_email,
            rules,
            registration_mode,
            authorized_fetch,
            max_note_length,
            max_file_size,
            allowed_file_types,
            properties,
        }
    }
}

impl Message for DomainCreateMessage {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::DomainCreateMessage(self.clone())
    }
}

/// Message for updating a domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainUpdateMessage {
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorized_fetch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_note_length: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_file_types: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
}

impl DomainUpdateMessage {
    /// Create a new domain update message
    pub fn new(
        domain: String,
        name: Option<String>,
        description: Option<String>,
        contact_email: Option<String>,
        rules: Option<Vec<String>>,
        registration_mode: Option<String>,
        authorized_fetch: Option<bool>,
        max_note_length: Option<i32>,
        max_file_size: Option<i64>,
        allowed_file_types: Option<Vec<String>>,
        properties: Option<Value>,
    ) -> Self {
        Self {
            domain,
            name,
            description,
            contact_email,
            rules,
            registration_mode,
            authorized_fetch,
            max_note_length,
            max_file_size,
            allowed_file_types,
            properties,
        }
    }
}

impl Message for DomainUpdateMessage {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::DomainUpdateMessage(self.clone())
    }
}

/// Message for deleting a domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDeleteMessage {
    pub domain: String,
    pub force: bool,
}

impl DomainDeleteMessage {
    /// Create a new domain deletion message
    pub fn new(domain: String, force: bool) -> Self {
        Self { domain, force }
    }
}

impl Message for DomainDeleteMessage {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::DomainDeleteMessage(self.clone())
    }
}

/// RPC request message for domain queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainRpcRequest {
    pub request_id: String,
    pub request_type: DomainRpcRequestType,
}

/// Types of domain RPC requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainRpcRequestType {
    ListDomains,
    GetDomain { domain: String },
}

impl DomainRpcRequest {
    /// Create a new domain list request
    pub fn list_domains(request_id: String) -> Self {
        Self {
            request_id,
            request_type: DomainRpcRequestType::ListDomains,
        }
    }

    /// Create a new domain get request
    pub fn get_domain(request_id: String, domain: String) -> Self {
        Self {
            request_id,
            request_type: DomainRpcRequestType::GetDomain { domain },
        }
    }
}

impl Message for DomainRpcRequest {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::DomainRpcRequest(self.clone())
    }
}

/// RPC response message for domain queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainRpcResponse {
    pub request_id: String,
    pub result: DomainRpcResult,
}

/// Results of domain RPC requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainRpcResult {
    DomainList { domains: Vec<DomainInfo> },
    DomainDetails { domain: Option<DomainInfo> },
    Error { message: String },
}

/// Domain information for RPC responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainInfo {
    pub domain: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub contact_email: Option<String>,
    pub registration_mode: String,
    pub authorized_fetch: bool,
    pub max_note_length: Option<i32>,
    pub max_file_size: Option<i64>,
    pub allowed_file_types: Option<Vec<String>>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl DomainRpcResponse {
    /// Create a domain list response
    pub fn domain_list(request_id: String, domains: Vec<DomainInfo>) -> Self {
        Self {
            request_id,
            result: DomainRpcResult::DomainList { domains },
        }
    }

    /// Create a domain details response
    pub fn domain_details(request_id: String, domain: Option<DomainInfo>) -> Self {
        Self {
            request_id,
            result: DomainRpcResult::DomainDetails { domain },
        }
    }

    /// Create an error response
    pub fn error(request_id: String, message: String) -> Self {
        Self {
            request_id,
            result: DomainRpcResult::Error { message },
        }
    }
}

impl Message for DomainRpcResponse {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::DomainRpcResponse(self.clone())
    }
}

/// Message format for incoming ActivityPub objects that need processing
///
/// This message type is used when forwarding received ActivityPub objects
/// to the incoming processing pipeline instead of storing them directly.
/// Uses RabbitMQ deliver-once semantics via publisher confirms and consumer acknowledgments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingObjectMessage {
    /// The raw object JSON as received
    pub object: Value,
    /// Type of the object (Note, Article, etc.)
    pub object_type: String,
    /// The actor ID that attributed this object
    pub attributed_to: String,
    /// The domain this object was received for
    pub target_domain: String,
    /// The username this object was addressed to (if any)
    pub target_username: Option<String>,
    /// Timestamp when the object was received
    pub received_at: String,
    /// Source IP or identifier for tracking
    pub source: Option<String>,
}

impl Message for IncomingObjectMessage {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::IncomingObjectMessage(self.clone())
    }
}

/// Message format for incoming ActivityPub activities that need processing
///
/// This message type is used when forwarding received ActivityPub activities
/// to the incoming processing pipeline.
/// Uses RabbitMQ deliver-once semantics via publisher confirms and consumer acknowledgments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingActivityMessage {
    /// The raw activity JSON as received
    pub activity: Value,
    /// Type of the activity (Create, Update, Delete, etc.)
    pub activity_type: String,
    /// The actor ID that performed this activity
    pub actor: String,
    /// The domain this activity was received for
    pub target_domain: String,
    /// The username this activity was addressed to (if any)
    pub target_username: Option<String>,
    /// Timestamp when the activity was received
    pub received_at: String,
    /// Source IP or identifier for tracking
    pub source: Option<String>,
}

impl Message for IncomingActivityMessage {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::IncomingActivityMessage(self.clone())
    }
}

/// Message for key generation requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyGenerateMessage {
    pub actor: String,
    pub algorithm: String,
    pub key_size: Option<u32>,
}

impl KeyGenerateMessage {
    /// Create a new key generation message
    pub fn new(actor: String, algorithm: String, key_size: Option<u32>) -> Self {
        Self {
            actor,
            algorithm,
            key_size,
        }
    }
}

impl Message for KeyGenerateMessage {
    fn to_message(&self) -> MessageEnum {
        MessageEnum::KeyGenerateMessage(self.clone())
    }
}
