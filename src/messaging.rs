//! Message types for inter-service communication
//!
//! This module defines message structures that are shared between
//! Oxifed services for communication via message queues.

use serde::{Deserialize, Serialize};

/// Message format for profile creation requests
///
/// This message type is used when sending profile creation requests
/// to message queues, following the same structure as the oxiadm CLI arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileCreateMessage {
    /// Name of the profile (used for display only)
    pub name: String,
    
    /// Subject of the profile (e.g. user@example.com or full acct:user@example.com)
    pub subject: String,
    
    /// Optional aliases for the profile (comma separated)
    pub aliases: Option<String>,
    
    /// Optional links to add to the profile (format: rel,href[,title][;rel2,href2,...])
    pub links: Option<String>,
}

/// Message format for profile edit requests
///
/// This message type is used when sending profile edit requests
/// to message queues, following the same structure as the oxiadm CLI arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEditMessage {
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
