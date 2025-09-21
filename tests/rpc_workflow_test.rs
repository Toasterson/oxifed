//! Comprehensive RPC workflow test
//!
//! This test simulates the complete RPC workflow from client request
//! to server response, verifying that the MessageEnum wrapper handling
//! works correctly throughout the entire process.

use oxifed::messaging::{
    DomainInfo, DomainRpcRequest, DomainRpcRequestType, DomainRpcResponse, DomainRpcResult,
    Message, MessageEnum,
};
use uuid::Uuid;

/// Simulate the complete RPC workflow for list domains
#[test]
fn test_complete_list_domains_rpc_workflow() {
    let request_id = Uuid::new_v4().to_string();

    // 1. Client creates RPC request
    let client_request = DomainRpcRequest::list_domains(request_id.clone());

    // 2. Client wraps request in MessageEnum and serializes (simulates sending over RabbitMQ)
    let client_message = client_request.to_message();
    let request_bytes = serde_json::to_vec(&client_message).unwrap();

    // 3. Server receives and parses the message
    let server_received_message: MessageEnum = serde_json::from_slice(&request_bytes).unwrap();

    // 4. Server extracts RPC request from MessageEnum
    let server_request = match server_received_message {
        MessageEnum::DomainRpcRequest(req) => req,
        _ => panic!("Expected DomainRpcRequest"),
    };

    // 5. Verify server got the correct request
    assert_eq!(server_request.request_id, request_id);
    assert!(matches!(
        server_request.request_type,
        DomainRpcRequestType::ListDomains
    ));

    // 6. Server processes request and creates response
    let test_domains = vec![
        DomainInfo {
            domain: "example1.com".to_string(),
            name: Some("Example 1".to_string()),
            description: Some("First test domain".to_string()),
            contact_email: Some("admin@example1.com".to_string()),
            registration_mode: "Approval".to_string(),
            authorized_fetch: true,
            max_note_length: Some(500),
            max_file_size: Some(10485760),
            allowed_file_types: Some(vec!["image/jpeg".to_string()]),
            status: "Active".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
        },
        DomainInfo {
            domain: "example2.com".to_string(),
            name: Some("Example 2".to_string()),
            description: Some("Second test domain".to_string()),
            contact_email: Some("admin@example2.com".to_string()),
            registration_mode: "Open".to_string(),
            authorized_fetch: false,
            max_note_length: Some(1000),
            max_file_size: Some(20971520),
            allowed_file_types: Some(vec!["image/png".to_string(), "image/gif".to_string()]),
            status: "Active".to_string(),
            created_at: "2024-01-02T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        },
    ];

    let server_response = DomainRpcResponse::domain_list(request_id.clone(), test_domains);

    // 7. Server wraps response in MessageEnum and serializes (simulates sending back over RabbitMQ)
    let server_message = server_response.to_message();
    let response_bytes = serde_json::to_vec(&server_message).unwrap();

    // 8. Client receives and parses the response
    let client_received_message: MessageEnum = serde_json::from_slice(&response_bytes).unwrap();

    // 9. Client extracts RPC response from MessageEnum
    let client_response = match client_received_message {
        MessageEnum::DomainRpcResponse(resp) => resp,
        _ => panic!("Expected DomainRpcResponse"),
    };

    // 10. Verify client got the correct response
    assert_eq!(client_response.request_id, request_id);

    if let DomainRpcResult::DomainList { domains } = client_response.result {
        assert_eq!(domains.len(), 2);

        // Verify first domain
        assert_eq!(domains[0].domain, "example1.com");
        assert_eq!(domains[0].name, Some("Example 1".to_string()));
        assert_eq!(domains[0].registration_mode, "Approval");
        assert!(domains[0].authorized_fetch);

        // Verify second domain
        assert_eq!(domains[1].domain, "example2.com");
        assert_eq!(domains[1].name, Some("Example 2".to_string()));
        assert_eq!(domains[1].registration_mode, "Open");
        assert!(!domains[1].authorized_fetch);
    } else {
        panic!("Expected DomainList result");
    }
}

/// Simulate the complete RPC workflow for get domain
#[test]
fn test_complete_get_domain_rpc_workflow() {
    let request_id = Uuid::new_v4().to_string();
    let domain_name = "specific.example".to_string();

    // 1. Client creates get domain RPC request
    let client_request = DomainRpcRequest::get_domain(request_id.clone(), domain_name.clone());

    // 2. Client sends request (MessageEnum wrapped)
    let request_bytes = serde_json::to_vec(&client_request.to_message()).unwrap();

    // 3. Server receives and processes
    let server_message: MessageEnum = serde_json::from_slice(&request_bytes).unwrap();
    let server_request = match server_message {
        MessageEnum::DomainRpcRequest(req) => req,
        _ => panic!("Expected DomainRpcRequest"),
    };

    // 4. Verify request details
    assert_eq!(server_request.request_id, request_id);
    if let DomainRpcRequestType::GetDomain { domain } = server_request.request_type {
        assert_eq!(domain, domain_name);
    } else {
        panic!("Expected GetDomain request type");
    }

    // 5. Server creates response with domain details
    let domain_info = DomainInfo {
        domain: domain_name.clone(),
        name: Some("Specific Domain".to_string()),
        description: Some("A specific test domain with detailed configuration".to_string()),
        contact_email: Some("admin@specific.example".to_string()),
        registration_mode: "Invite".to_string(),
        authorized_fetch: true,
        max_note_length: Some(2000),
        max_file_size: Some(52428800),
        allowed_file_types: Some(vec![
            "image/jpeg".to_string(),
            "image/png".to_string(),
            "image/gif".to_string(),
            "video/mp4".to_string(),
        ]),
        status: "Active".to_string(),
        created_at: "2024-01-01T00:00:00Z".to_string(),
        updated_at: "2024-01-15T12:30:45Z".to_string(),
    };

    let server_response = DomainRpcResponse::domain_details(request_id.clone(), Some(domain_info));

    // 6. Server sends response (MessageEnum wrapped)
    let response_bytes = serde_json::to_vec(&server_response.to_message()).unwrap();

    // 7. Client receives and processes response
    let client_message: MessageEnum = serde_json::from_slice(&response_bytes).unwrap();
    let client_response = match client_message {
        MessageEnum::DomainRpcResponse(resp) => resp,
        _ => panic!("Expected DomainRpcResponse"),
    };

    // 8. Verify response details
    assert_eq!(client_response.request_id, request_id);

    if let DomainRpcResult::DomainDetails { domain } = client_response.result {
        let domain = *domain;
        assert!(domain.is_some());
        let domain_info = domain.unwrap();

        assert_eq!(domain_info.domain, domain_name);
        assert_eq!(domain_info.name, Some("Specific Domain".to_string()));
        assert_eq!(domain_info.registration_mode, "Invite");
        assert!(domain_info.authorized_fetch);
        assert_eq!(domain_info.max_note_length, Some(2000));
        assert_eq!(domain_info.max_file_size, Some(52428800));
        assert_eq!(domain_info.allowed_file_types.as_ref().unwrap().len(), 4);
        assert!(
            domain_info
                .allowed_file_types
                .as_ref()
                .unwrap()
                .contains(&"video/mp4".to_string())
        );
    } else {
        panic!("Expected DomainDetails result");
    }
}

/// Test error handling in the RPC workflow
#[test]
fn test_rpc_error_workflow() {
    let request_id = Uuid::new_v4().to_string();

    // 1. Client creates request for non-existent domain
    let client_request =
        DomainRpcRequest::get_domain(request_id.clone(), "nonexistent.example".to_string());

    // 2. Simulate request transmission
    let request_bytes = serde_json::to_vec(&client_request.to_message()).unwrap();
    let server_message: MessageEnum = serde_json::from_slice(&request_bytes).unwrap();
    let _server_request = match server_message {
        MessageEnum::DomainRpcRequest(req) => req,
        _ => panic!("Expected DomainRpcRequest"),
    };

    // 3. Server processes and creates error response
    let error_message = "Domain not found in database".to_string();
    let server_response = DomainRpcResponse::error(request_id.clone(), error_message.clone());

    // 4. Simulate response transmission
    let response_bytes = serde_json::to_vec(&server_response.to_message()).unwrap();
    let client_message: MessageEnum = serde_json::from_slice(&response_bytes).unwrap();
    let client_response = match client_message {
        MessageEnum::DomainRpcResponse(resp) => resp,
        _ => panic!("Expected DomainRpcResponse"),
    };

    // 5. Verify error response
    assert_eq!(client_response.request_id, request_id);

    if let DomainRpcResult::Error { message } = client_response.result {
        assert_eq!(message, error_message);
    } else {
        panic!("Expected Error result");
    }
}

/// Test domain not found scenario
#[test]
fn test_domain_not_found_workflow() {
    let request_id = Uuid::new_v4().to_string();

    // 1. Client requests specific domain
    let client_request =
        DomainRpcRequest::get_domain(request_id.clone(), "missing.example".to_string());

    // 2. Simulate transmission and processing
    let request_bytes = serde_json::to_vec(&client_request.to_message()).unwrap();
    let server_message: MessageEnum = serde_json::from_slice(&request_bytes).unwrap();

    if let MessageEnum::DomainRpcRequest(_server_request) = server_message {
        // 3. Server responds with None (domain not found, but no error)
        let server_response = DomainRpcResponse::domain_details(request_id.clone(), None);

        // 4. Simulate response transmission
        let response_bytes = serde_json::to_vec(&server_response.to_message()).unwrap();
        let client_message: MessageEnum = serde_json::from_slice(&response_bytes).unwrap();

        if let MessageEnum::DomainRpcResponse(client_response) = client_message {
            // 5. Verify not found response
            assert_eq!(client_response.request_id, request_id);

            if let DomainRpcResult::DomainDetails { domain } = client_response.result {
                let domain = *domain;
                assert!(domain.is_none());
            } else {
                panic!("Expected DomainDetails result");
            }
        } else {
            panic!("Expected DomainRpcResponse");
        }
    } else {
        panic!("Expected DomainRpcRequest");
    }
}

/// Test that the workflow handles correlation IDs correctly
#[test]
fn test_correlation_id_handling() {
    let request_id_1 = "req-001".to_string();
    let request_id_2 = "req-002".to_string();

    // Create two different requests with different IDs
    let request_1 = DomainRpcRequest::list_domains(request_id_1.clone());
    let request_2 = DomainRpcRequest::get_domain(request_id_2.clone(), "example.com".to_string());

    // Serialize both requests
    let bytes_1 = serde_json::to_vec(&request_1.to_message()).unwrap();
    let bytes_2 = serde_json::to_vec(&request_2.to_message()).unwrap();

    // Server processes both
    let msg_1: MessageEnum = serde_json::from_slice(&bytes_1).unwrap();
    let msg_2: MessageEnum = serde_json::from_slice(&bytes_2).unwrap();

    if let (MessageEnum::DomainRpcRequest(req_1), MessageEnum::DomainRpcRequest(req_2)) =
        (msg_1, msg_2)
    {
        // Create responses with matching request IDs
        let resp_1 = DomainRpcResponse::domain_list(req_1.request_id.clone(), vec![]);
        let resp_2 = DomainRpcResponse::domain_details(req_2.request_id.clone(), None);

        // Serialize responses
        let resp_bytes_1 = serde_json::to_vec(&resp_1.to_message()).unwrap();
        let resp_bytes_2 = serde_json::to_vec(&resp_2.to_message()).unwrap();

        // Client receives responses
        let client_msg_1: MessageEnum = serde_json::from_slice(&resp_bytes_1).unwrap();
        let client_msg_2: MessageEnum = serde_json::from_slice(&resp_bytes_2).unwrap();

        if let (
            MessageEnum::DomainRpcResponse(client_resp_1),
            MessageEnum::DomainRpcResponse(client_resp_2),
        ) = (client_msg_1, client_msg_2)
        {
            // Verify correlation IDs match
            assert_eq!(client_resp_1.request_id, request_id_1);
            assert_eq!(client_resp_2.request_id, request_id_2);

            // Verify response types match request types
            assert!(matches!(
                client_resp_1.result,
                DomainRpcResult::DomainList { .. }
            ));
            assert!(matches!(
                client_resp_2.result,
                DomainRpcResult::DomainDetails { .. }
            ));
        } else {
            panic!("Expected DomainRpcResponse messages");
        }
    } else {
        panic!("Expected DomainRpcRequest messages");
    }
}
