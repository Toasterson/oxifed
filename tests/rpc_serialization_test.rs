//! Test RPC message serialization with MessageEnum wrapper
//!
//! This test verifies that RPC messages are correctly serialized and deserialized
//! when wrapped in MessageEnum, which was the source of the parsing error.

use oxifed::messaging::{
    DomainInfo, DomainRpcRequest, DomainRpcRequestType, DomainRpcResponse, DomainRpcResult,
    Message, MessageEnum,
};
use uuid::Uuid;

#[test]
fn test_rpc_request_with_message_enum_wrapper() {
    let request_id = Uuid::new_v4().to_string();

    // Create RPC request
    let rpc_request = DomainRpcRequest::list_domains(request_id.clone());

    // Wrap in MessageEnum (this is what gets sent over RabbitMQ)
    let message_enum = rpc_request.to_message();

    // Serialize to JSON (simulating RabbitMQ message)
    let json_data = serde_json::to_vec(&message_enum).unwrap();

    // Deserialize from JSON (simulating server-side parsing)
    let parsed_message: MessageEnum = serde_json::from_slice(&json_data).unwrap();

    // Extract RPC request from MessageEnum
    if let MessageEnum::DomainRpcRequest(parsed_request) = parsed_message {
        assert_eq!(parsed_request.request_id, request_id);
        matches!(
            parsed_request.request_type,
            DomainRpcRequestType::ListDomains
        );
    } else {
        panic!("Expected DomainRpcRequest in MessageEnum");
    }
}

#[test]
fn test_rpc_response_with_message_enum_wrapper() {
    let request_id = Uuid::new_v4().to_string();

    // Create test domain info
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

    // Create RPC response
    let rpc_response =
        DomainRpcResponse::domain_list(request_id.clone(), vec![domain_info.clone()]);

    // Wrap in MessageEnum (this is what gets sent back over RabbitMQ)
    let message_enum = rpc_response.to_message();

    // Serialize to JSON
    let json_data = serde_json::to_vec(&message_enum).unwrap();

    // Deserialize from JSON (simulating client-side parsing)
    let parsed_message: MessageEnum = serde_json::from_slice(&json_data).unwrap();

    // Extract RPC response from MessageEnum
    if let MessageEnum::DomainRpcResponse(parsed_response) = parsed_message {
        assert_eq!(parsed_response.request_id, request_id);
        if let DomainRpcResult::DomainList { domains } = parsed_response.result {
            assert_eq!(domains.len(), 1);
            assert_eq!(domains[0].domain, "test.example");
        } else {
            panic!("Expected DomainList result");
        }
    } else {
        panic!("Expected DomainRpcResponse in MessageEnum");
    }
}

#[test]
fn test_get_domain_rpc_request_serialization() {
    let request_id = Uuid::new_v4().to_string();
    let domain_name = "specific.example".to_string();

    // Create get domain RPC request
    let rpc_request = DomainRpcRequest::get_domain(request_id.clone(), domain_name.clone());

    // Serialize through MessageEnum wrapper
    let json_data = serde_json::to_vec(&rpc_request.to_message()).unwrap();

    // Verify the JSON contains expected fields
    let json_string = String::from_utf8(json_data.clone()).unwrap();
    assert!(json_string.contains("DomainRpcRequest"));
    assert!(json_string.contains(&request_id));
    assert!(json_string.contains(&domain_name));

    // Parse back and verify
    let parsed_message: MessageEnum = serde_json::from_slice(&json_data).unwrap();
    if let MessageEnum::DomainRpcRequest(parsed_request) = parsed_message {
        assert_eq!(parsed_request.request_id, request_id);
        if let DomainRpcRequestType::GetDomain { domain } = parsed_request.request_type {
            assert_eq!(domain, domain_name);
        } else {
            panic!("Expected GetDomain request type");
        }
    } else {
        panic!("Expected DomainRpcRequest in MessageEnum");
    }
}

#[test]
fn test_error_response_serialization() {
    let request_id = Uuid::new_v4().to_string();
    let error_message = "Database connection failed".to_string();

    // Create error response
    let rpc_response = DomainRpcResponse::error(request_id.clone(), error_message.clone());

    // Serialize through MessageEnum wrapper
    let json_data = serde_json::to_vec(&rpc_response.to_message()).unwrap();

    // Parse back and verify
    let parsed_message: MessageEnum = serde_json::from_slice(&json_data).unwrap();
    if let MessageEnum::DomainRpcResponse(parsed_response) = parsed_message {
        assert_eq!(parsed_response.request_id, request_id);
        if let DomainRpcResult::Error { message } = parsed_response.result {
            assert_eq!(message, error_message);
        } else {
            panic!("Expected Error result");
        }
    } else {
        panic!("Expected DomainRpcResponse in MessageEnum");
    }
}

#[test]
fn test_domain_details_not_found_serialization() {
    let request_id = Uuid::new_v4().to_string();

    // Create domain details response with None (not found)
    let rpc_response = DomainRpcResponse::domain_details(request_id.clone(), None);

    // Serialize through MessageEnum wrapper
    let json_data = serde_json::to_vec(&rpc_response.to_message()).unwrap();

    // Parse back and verify
    let parsed_message: MessageEnum = serde_json::from_slice(&json_data).unwrap();
    if let MessageEnum::DomainRpcResponse(parsed_response) = parsed_message {
        assert_eq!(parsed_response.request_id, request_id);
        if let DomainRpcResult::DomainDetails { domain } = parsed_response.result {
            let domain = *domain;
            assert!(domain.is_none());
        } else {
            panic!("Expected DomainDetails result");
        }
    } else {
        panic!("Expected DomainRpcResponse in MessageEnum");
    }
}
