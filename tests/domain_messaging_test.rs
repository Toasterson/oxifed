//! Tests for domain messaging functionality
//!
//! This module tests the domain message creation and serialization
//! to ensure they work correctly with the RabbitMQ messaging system.

use oxifed::messaging::{DomainCreateMessage, DomainUpdateMessage, DomainDeleteMessage, Message, MessageEnum};
use serde_json;

#[test]
fn test_domain_create_message_serialization() {
    let message = DomainCreateMessage::new(
        "example.com".to_string(),
        Some("Example Domain".to_string()),
        Some("A test domain".to_string()),
        Some("admin@example.com".to_string()),
        Some(vec!["No spam".to_string(), "Be respectful".to_string()]),
        Some("approval".to_string()),
        Some(true),
        Some(500),
        Some(10485760),
        Some(vec!["image/jpeg".to_string(), "image/png".to_string()]),
        None,
    );

    // Test that the message can be serialized to JSON
    let json = serde_json::to_string(&message.to_message()).unwrap();
    assert!(json.contains("example.com"));
    assert!(json.contains("Example Domain"));
    assert!(json.contains("admin@example.com"));
    assert!(json.contains("approval"));
    
    // Test that it can be deserialized back
    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    if let MessageEnum::DomainCreateMessage(domain_msg) = deserialized {
        assert_eq!(domain_msg.domain, "example.com");
        assert_eq!(domain_msg.name, Some("Example Domain".to_string()));
        assert_eq!(domain_msg.contact_email, Some("admin@example.com".to_string()));
        assert_eq!(domain_msg.registration_mode, Some("approval".to_string()));
        assert_eq!(domain_msg.authorized_fetch, Some(true));
        assert_eq!(domain_msg.max_note_length, Some(500));
        assert_eq!(domain_msg.max_file_size, Some(10485760));
    } else {
        panic!("Expected DomainCreateMessage");
    }
}

#[test]
fn test_domain_update_message_serialization() {
    let message = DomainUpdateMessage::new(
        "example.com".to_string(),
        Some("Updated Domain".to_string()),
        None,
        None,
        None,
        Some("open".to_string()),
        Some(false),
        Some(1000),
        None,
        None,
        None,
    );

    let json = serde_json::to_string(&message.to_message()).unwrap();
    assert!(json.contains("example.com"));
    assert!(json.contains("Updated Domain"));
    assert!(json.contains("open"));
    
    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    if let MessageEnum::DomainUpdateMessage(domain_msg) = deserialized {
        assert_eq!(domain_msg.domain, "example.com");
        assert_eq!(domain_msg.name, Some("Updated Domain".to_string()));
        assert_eq!(domain_msg.registration_mode, Some("open".to_string()));
        assert_eq!(domain_msg.authorized_fetch, Some(false));
        assert_eq!(domain_msg.max_note_length, Some(1000));
    } else {
        panic!("Expected DomainUpdateMessage");
    }
}

#[test]
fn test_domain_delete_message_serialization() {
    let message = DomainDeleteMessage::new("example.com".to_string(), true);

    let json = serde_json::to_string(&message.to_message()).unwrap();
    assert!(json.contains("example.com"));
    assert!(json.contains("true"));
    
    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    if let MessageEnum::DomainDeleteMessage(domain_msg) = deserialized {
        assert_eq!(domain_msg.domain, "example.com");
        assert_eq!(domain_msg.force, true);
    } else {
        panic!("Expected DomainDeleteMessage");
    }
}

#[test]
fn test_domain_create_message_minimal() {
    // Test with minimal required fields
    let message = DomainCreateMessage::new(
        "minimal.com".to_string(),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );

    let json = serde_json::to_string(&message.to_message()).unwrap();
    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    
    if let MessageEnum::DomainCreateMessage(domain_msg) = deserialized {
        assert_eq!(domain_msg.domain, "minimal.com");
        assert_eq!(domain_msg.name, None);
        assert_eq!(domain_msg.description, None);
        assert_eq!(domain_msg.contact_email, None);
        assert_eq!(domain_msg.registration_mode, None);
        assert_eq!(domain_msg.authorized_fetch, None);
    } else {
        panic!("Expected DomainCreateMessage");
    }
}

#[test]
fn test_domain_message_with_custom_properties() {
    use serde_json::json;
    
    let custom_props = json!({
        "theme": "dark",
        "features": ["polls", "reactions"],
        "limits": {
            "max_poll_options": 8,
            "poll_duration_hours": 168
        }
    });

    let message = DomainCreateMessage::new(
        "custom.com".to_string(),
        Some("Custom Domain".to_string()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(custom_props.clone()),
    );

    let json = serde_json::to_string(&message.to_message()).unwrap();
    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    
    if let MessageEnum::DomainCreateMessage(domain_msg) = deserialized {
        assert_eq!(domain_msg.domain, "custom.com");
        assert_eq!(domain_msg.properties, Some(custom_props));
    } else {
        panic!("Expected DomainCreateMessage");
    }
}