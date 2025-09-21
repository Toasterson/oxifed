//! Integration tests for the complete domain management system
//!
//! This module tests the full domain management workflow including
//! message creation, RPC requests/responses, and error handling.

use oxifed::messaging::{
    DomainCreateMessage, DomainDeleteMessage, DomainInfo, DomainRpcRequest, DomainRpcRequestType,
    DomainRpcResponse, DomainRpcResult, DomainUpdateMessage, Message, MessageEnum,
};
use serde_json;
use uuid::Uuid;

#[test]
fn test_complete_domain_lifecycle_messages() {
    // Test domain creation message
    let create_msg = DomainCreateMessage::new(
        "test.example".to_string(),
        Some("Test Domain".to_string()),
        Some("A comprehensive test domain".to_string()),
        Some("admin@test.example".to_string()),
        Some(vec!["No spam".to_string(), "Be respectful".to_string()]),
        Some("approval".to_string()),
        Some(true),
        Some(500),
        Some(10485760),
        Some(vec!["image/jpeg".to_string(), "image/png".to_string()]),
        None,
    );

    let create_json = serde_json::to_string(&create_msg.to_message()).unwrap();
    assert!(create_json.contains("test.example"));
    assert!(create_json.contains("DomainCreateMessage"));

    // Test domain update message
    let update_msg = DomainUpdateMessage::new(
        "test.example".to_string(),
        Some("Updated Test Domain".to_string()),
        Some("An updated test domain".to_string()),
        None,
        None,
        Some("open".to_string()),
        Some(false),
        Some(1000),
        None,
        None,
        None,
    );

    let update_json = serde_json::to_string(&update_msg.to_message()).unwrap();
    assert!(update_json.contains("test.example"));
    assert!(update_json.contains("Updated Test Domain"));

    // Test domain deletion message
    let delete_msg = DomainDeleteMessage::new("test.example".to_string(), false);
    let delete_json = serde_json::to_string(&delete_msg.to_message()).unwrap();
    assert!(delete_json.contains("test.example"));
    assert!(delete_json.contains("false"));
}

#[test]
fn test_rpc_request_response_workflow() {
    let request_id = Uuid::new_v4().to_string();

    // Test list domains RPC workflow
    let list_request = DomainRpcRequest::list_domains(request_id.clone());
    let request_json = serde_json::to_string(&list_request.to_message()).unwrap();

    // Verify request serialization
    let deserialized_request: MessageEnum = serde_json::from_str(&request_json).unwrap();
    if let MessageEnum::DomainRpcRequest(req) = deserialized_request {
        assert_eq!(req.request_id, request_id);
        matches!(req.request_type, DomainRpcRequestType::ListDomains);
    } else {
        panic!("Expected DomainRpcRequest");
    }

    // Create mock response
    let domain_info = DomainInfo {
        domain: "test.example".to_string(),
        name: Some("Test Domain".to_string()),
        description: Some("A test domain".to_string()),
        contact_email: Some("admin@test.example".to_string()),
        registration_mode: "Approval".to_string(),
        authorized_fetch: true,
        max_note_length: Some(500),
        max_file_size: Some(10485760),
        allowed_file_types: Some(vec!["image/jpeg".to_string()]),
        status: "Active".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-01T00:00:00Z".to_string(),
    };

    let list_response =
        DomainRpcResponse::domain_list(request_id.clone(), vec![domain_info.clone()]);

    let response_json = serde_json::to_string(&list_response.to_message()).unwrap();

    // Verify response serialization
    let deserialized_response: MessageEnum = serde_json::from_str(&response_json).unwrap();
    if let MessageEnum::DomainRpcResponse(resp) = deserialized_response {
        assert_eq!(resp.request_id, request_id);
        if let DomainRpcResult::DomainList { domains } = resp.result {
            assert_eq!(domains.len(), 1);
            assert_eq!(domains[0].domain, "test.example");
            assert_eq!(domains[0].name, Some("Test Domain".to_string()));
            assert_eq!(domains[0].authorized_fetch, true);
        } else {
            panic!("Expected DomainList result");
        }
    } else {
        panic!("Expected DomainRpcResponse");
    }
}

#[test]
fn test_get_domain_rpc_workflow() {
    let request_id = Uuid::new_v4().to_string();
    let domain_name = "specific.example".to_string();

    // Test get domain RPC request
    let get_request = DomainRpcRequest::get_domain(request_id.clone(), domain_name.clone());
    let request_json = serde_json::to_string(&get_request.to_message()).unwrap();

    // Verify request
    let deserialized_request: MessageEnum = serde_json::from_str(&request_json).unwrap();
    if let MessageEnum::DomainRpcRequest(req) = deserialized_request {
        assert_eq!(req.request_id, request_id);
        if let DomainRpcRequestType::GetDomain { domain } = req.request_type {
            assert_eq!(domain, domain_name);
        } else {
            panic!("Expected GetDomain request type");
        }
    } else {
        panic!("Expected DomainRpcRequest");
    }

    // Test successful response
    let domain_info = DomainInfo {
        domain: domain_name.clone(),
        name: Some("Specific Domain".to_string()),
        description: Some("A specific test domain".to_string()),
        contact_email: Some("admin@specific.example".to_string()),
        registration_mode: "Open".to_string(),
        authorized_fetch: false,
        max_note_length: Some(1000),
        max_file_size: Some(20971520),
        allowed_file_types: Some(vec![
            "image/jpeg".to_string(),
            "image/png".to_string(),
            "image/gif".to_string(),
        ]),
        status: "Active".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-02T00:00:00Z".to_string(),
    };

    let success_response = DomainRpcResponse::domain_details(request_id.clone(), Some(domain_info));

    let response_json = serde_json::to_string(&success_response.to_message()).unwrap();
    let deserialized_response: MessageEnum = serde_json::from_str(&response_json).unwrap();

    if let MessageEnum::DomainRpcResponse(resp) = deserialized_response {
        assert_eq!(resp.request_id, request_id);
        if let DomainRpcResult::DomainDetails { domain } = resp.result {
            let domain = *domain;
            assert!(domain.is_some());
            let domain_info = domain.unwrap();
            assert_eq!(domain_info.domain, domain_name);
            assert_eq!(domain_info.registration_mode, "Open");
            assert_eq!(domain_info.authorized_fetch, false);
            assert_eq!(domain_info.max_note_length, Some(1000));
        } else {
            panic!("Expected DomainDetails result");
        }
    }

    // Test not found response
    let not_found_response = DomainRpcResponse::domain_details(request_id.clone(), None);
    let not_found_json = serde_json::to_string(&not_found_response.to_message()).unwrap();
    let deserialized_not_found: MessageEnum = serde_json::from_str(&not_found_json).unwrap();

    if let MessageEnum::DomainRpcResponse(resp) = deserialized_not_found {
        if let DomainRpcResult::DomainDetails { domain } = resp.result {
            let domain = *domain;
            assert!(domain.is_none());
        } else {
            panic!("Expected DomainDetails result");
        }
    }
}

#[test]
fn test_error_handling_scenarios() {
    let request_id = Uuid::new_v4().to_string();

    // Test database error response
    let error_response =
        DomainRpcResponse::error(request_id.clone(), "Database connection failed".to_string());

    let error_json = serde_json::to_string(&error_response.to_message()).unwrap();
    let deserialized_error: MessageEnum = serde_json::from_str(&error_json).unwrap();

    if let MessageEnum::DomainRpcResponse(resp) = deserialized_error {
        assert_eq!(resp.request_id, request_id);
        if let DomainRpcResult::Error { message } = resp.result {
            assert_eq!(message, "Database connection failed");
        } else {
            panic!("Expected Error result");
        }
    } else {
        panic!("Expected DomainRpcResponse");
    }

    // Test constraint violation error for domain creation
    let constraint_error =
        DomainRpcResponse::error(request_id.clone(), "Domain already exists".to_string());

    let constraint_json = serde_json::to_string(&constraint_error.to_message()).unwrap();
    assert!(constraint_json.contains("Domain already exists"));
}

#[test]
fn test_domain_info_comprehensive_fields() {
    let comprehensive_domain = DomainInfo {
        domain: "comprehensive.test".to_string(),
        name: Some("Comprehensive Test Domain".to_string()),
        description: Some("A domain with all possible fields populated for testing".to_string()),
        contact_email: Some("comprehensive-admin@comprehensive.test".to_string()),
        registration_mode: "Invite".to_string(),
        authorized_fetch: true,
        max_note_length: Some(2000),
        max_file_size: Some(52428800), // 50MB
        allowed_file_types: Some(vec![
            "image/jpeg".to_string(),
            "image/png".to_string(),
            "image/gif".to_string(),
            "image/webp".to_string(),
            "video/mp4".to_string(),
            "audio/mpeg".to_string(),
        ]),
        status: "Active".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-15T12:30:45Z".to_string(),
    };

    // Test serialization and deserialization
    let domain_json = serde_json::to_string(&comprehensive_domain).unwrap();
    let deserialized_domain: DomainInfo = serde_json::from_str(&domain_json).unwrap();

    assert_eq!(deserialized_domain.domain, "comprehensive.test");
    assert_eq!(
        deserialized_domain.name,
        Some("Comprehensive Test Domain".to_string())
    );
    assert_eq!(deserialized_domain.registration_mode, "Invite");
    assert_eq!(deserialized_domain.authorized_fetch, true);
    assert_eq!(deserialized_domain.max_note_length, Some(2000));
    assert_eq!(deserialized_domain.max_file_size, Some(52428800));
    assert_eq!(
        deserialized_domain
            .allowed_file_types
            .as_ref()
            .unwrap()
            .len(),
        6
    );
    assert!(
        deserialized_domain
            .allowed_file_types
            .as_ref()
            .unwrap()
            .contains(&"video/mp4".to_string())
    );
}

#[test]
fn test_message_enum_completeness() {
    // Verify all domain-related message types are included in MessageEnum
    let create_msg = DomainCreateMessage::new(
        "test.com".to_string(),
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
    let update_msg = DomainUpdateMessage::new(
        "test.com".to_string(),
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
    let delete_msg = DomainDeleteMessage::new("test.com".to_string(), false);
    let rpc_request = DomainRpcRequest::list_domains("req-123".to_string());
    let rpc_response = DomainRpcResponse::error("req-123".to_string(), "Test error".to_string());

    // Test that all message types can be converted to MessageEnum
    let _create_enum = create_msg.to_message();
    let _update_enum = update_msg.to_message();
    let _delete_enum = delete_msg.to_message();
    let _rpc_req_enum = rpc_request.to_message();
    let _rpc_resp_enum = rpc_response.to_message();

    // Test JSON serialization of all types
    let messages = vec![
        create_msg.to_message(),
        update_msg.to_message(),
        delete_msg.to_message(),
        rpc_request.to_message(),
        rpc_response.to_message(),
    ];

    for message in messages {
        let json = serde_json::to_string(&message).unwrap();
        let _deserialized: MessageEnum = serde_json::from_str(&json).unwrap();
    }
}
