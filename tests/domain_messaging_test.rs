//! Tests for domain messaging functionality
//!
//! This module tests the domain message creation and serialization
//! to ensure they work correctly with the RabbitMQ messaging system.

use oxifed::messaging::{
    DomainCreateMessage, DomainDeleteMessage, DomainUpdateMessage, Message, MessageEnum,
};

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
        assert_eq!(
            domain_msg.contact_email,
            Some("admin@example.com".to_string())
        );
        assert_eq!(domain_msg.registration_mode, Some("approval".to_string()));
        assert_eq!(domain_msg.authorized_fetch, Some(true));
        assert_eq!(domain_msg.max_note_length, Some(500));
        assert_eq!(domain_msg.max_file_size, Some(10485760));
    } else {
        panic!("Expected DomainCreateMessage");
    }
}

#[test]
fn test_domain_rpc_request_serialization() {
    use oxifed::messaging::{DomainRpcRequest, DomainRpcRequestType};

    // Test list domains request
    let list_request = DomainRpcRequest::list_domains("req-123".to_string());
    let json = serde_json::to_string(&list_request.to_message()).unwrap();

    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    if let MessageEnum::DomainRpcRequest(rpc_req) = deserialized {
        assert_eq!(rpc_req.request_id, "req-123");
        if let DomainRpcRequestType::ListDomains = rpc_req.request_type {
            // Expected
        } else {
            panic!("Expected ListDomains request type");
        }
    } else {
        panic!("Expected DomainRpcRequest");
    }

    // Test get domain request
    let get_request =
        DomainRpcRequest::get_domain("req-456".to_string(), "example.com".to_string());
    let json = serde_json::to_string(&get_request.to_message()).unwrap();

    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    if let MessageEnum::DomainRpcRequest(rpc_req) = deserialized {
        assert_eq!(rpc_req.request_id, "req-456");
        if let DomainRpcRequestType::GetDomain { domain } = rpc_req.request_type {
            assert_eq!(domain, "example.com");
        } else {
            panic!("Expected GetDomain request type");
        }
    } else {
        panic!("Expected DomainRpcRequest");
    }
}

#[test]
fn test_domain_rpc_response_serialization() {
    use oxifed::messaging::{DomainInfo, DomainRpcResponse, DomainRpcResult};

    // Create test domain info
    let domain_info = DomainInfo {
        domain: "test.com".to_string(),
        name: Some("Test Domain".to_string()),
        description: Some("A test domain".to_string()),
        contact_email: Some("admin@test.com".to_string()),
        registration_mode: "Approval".to_string(),
        authorized_fetch: true,
        max_note_length: Some(500),
        max_file_size: Some(10485760),
        allowed_file_types: Some(vec!["image/jpeg".to_string()]),
        status: "Active".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    // Test domain list response
    let list_response =
        DomainRpcResponse::domain_list("req-123".to_string(), vec![domain_info.clone()]);
    let json = serde_json::to_string(&list_response.to_message()).unwrap();

    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    if let MessageEnum::DomainRpcResponse(rpc_resp) = deserialized {
        assert_eq!(rpc_resp.request_id, "req-123");
        if let DomainRpcResult::DomainList { domains } = rpc_resp.result {
            assert_eq!(domains.len(), 1);
            assert_eq!(domains[0].domain, "test.com");
        } else {
            panic!("Expected DomainList result");
        }
    } else {
        panic!("Expected DomainRpcResponse");
    }

    // Test domain details response
    let details_response =
        DomainRpcResponse::domain_details("req-456".to_string(), Some(domain_info));
    let json = serde_json::to_string(&details_response.to_message()).unwrap();

    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    if let MessageEnum::DomainRpcResponse(rpc_resp) = deserialized {
        assert_eq!(rpc_resp.request_id, "req-456");
        if let DomainRpcResult::DomainDetails { domain } = rpc_resp.result {
            let domain = *domain;
            assert!(domain.is_some());
            assert_eq!(domain.unwrap().domain, "test.com");
        } else {
            panic!("Expected DomainDetails result");
        }
    } else {
        panic!("Expected DomainRpcResponse");
    }

    // Test error response
    let error_response =
        DomainRpcResponse::error("req-789".to_string(), "Database error".to_string());
    let json = serde_json::to_string(&error_response.to_message()).unwrap();

    let deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    if let MessageEnum::DomainRpcResponse(rpc_resp) = deserialized {
        assert_eq!(rpc_resp.request_id, "req-789");
        if let DomainRpcResult::Error { message } = rpc_resp.result {
            assert_eq!(message, "Database error");
        } else {
            panic!("Expected Error result");
        }
    } else {
        panic!("Expected DomainRpcResponse");
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
        assert!(domain_msg.force);
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
