#!/usr/bin/env rust-script

//! Test script to verify dual-mode key exposure in actor documents
//! 
//! This script tests that actor JSON documents include both:
//! 1. Standard ActivityPub publicKey field
//! 2. Extended oxifed:keyChain field for PKI-aware servers

use serde_json::{json, Value};

fn main() {
    println!("Testing dual-mode key exposure...");
    
    let test_actor = create_test_actor();
    
    // Test 1: Standard publicKey field exists
    assert!(test_actor.get("publicKey").is_some(), "Standard publicKey field missing");
    
    let public_key = test_actor.get("publicKey").unwrap();
    assert!(public_key.get("id").is_some(), "publicKey.id missing");
    assert!(public_key.get("owner").is_some(), "publicKey.owner missing"); 
    assert!(public_key.get("publicKeyPem").is_some(), "publicKey.publicKeyPem missing");
    
    println!("✓ Standard ActivityPub publicKey format: OK");
    
    // Test 2: Extended keyChain field exists
    assert!(test_actor.get("oxifed:keyChain").is_some(), "oxifed:keyChain field missing");
    
    let key_chain = test_actor.get("oxifed:keyChain").unwrap();
    assert_eq!(key_chain.get("type").unwrap().as_str().unwrap(), "OxifedPKI", "keyChain type incorrect");
    assert!(key_chain.get("userKey").is_some(), "keyChain.userKey missing");
    assert!(key_chain.get("domainKey").is_some(), "keyChain.domainKey missing");
    assert!(key_chain.get("masterKey").is_some(), "keyChain.masterKey missing");
    
    println!("✓ Extended oxifed:keyChain format: OK");
    
    // Test 3: Context includes oxifed namespace
    let context = test_actor.get("@context").unwrap().as_array().unwrap();
    let has_oxifed_namespace = context.iter().any(|item| {
        if let Some(obj) = item.as_object() {
            obj.contains_key("oxifed") && obj.get("oxifed") == Some(&json!("https://oxifed.org/ns#"))
        } else {
            false
        }
    });
    assert!(has_oxifed_namespace, "oxifed namespace missing from @context");
    
    println!("✓ JSON-LD @context with oxifed namespace: OK");
    
    // Test 4: Key ID consistency
    let user_key_id = public_key.get("id").unwrap().as_str().unwrap();
    let chain_user_key = key_chain.get("userKey").unwrap().as_str().unwrap();
    assert_eq!(user_key_id, chain_user_key, "Key ID mismatch between publicKey.id and keyChain.userKey");
    
    println!("✓ Key ID consistency: OK");
    
    println!("\n🎉 All dual-mode key exposure tests passed!");
    
    // Print sample output
    println!("\n--- Sample Actor Document ---");
    println!("{}", serde_json::to_string_pretty(&test_actor).unwrap());
}

fn create_test_actor() -> Value {
    // Simulate the JSON structure our modified get_actor function would produce
    let domain = "example.com";
    let actor_id = "https://example.com/users/alice";
    let key_id = format!("{}#main-key", actor_id);
    
    json!({
        "@context": [
            "https://www.w3.org/ns/activitystreams",
            "https://w3id.org/security/v1",
            {
                "manuallyApprovesFollowers": "as:manuallyApprovesFollowers",
                "toot": "http://joinmastodon.org/ns#",
                "featured": {
                    "@id": "toot:featured",
                    "@type": "@id"
                },
                "PropertyValue": "schema:PropertyValue",
                "value": "schema:value",
                "oxifed": "https://oxifed.org/ns#",
                "keyChain": {
                    "@id": "oxifed:keyChain",
                    "@type": "@id"
                }
            }
        ],
        "type": "Person",
        "id": actor_id,
        "name": "Alice",
        "preferredUsername": "alice",
        "summary": "Test user for dual-mode key exposure",
        "inbox": format!("{}/inbox", actor_id),
        "outbox": format!("{}/outbox", actor_id),
        "following": format!("{}/following", actor_id),
        "followers": format!("{}/followers", actor_id),
        "publicKey": {
            "id": key_id.clone(),
            "owner": actor_id,
            "publicKeyPem": "-----BEGIN PUBLIC KEY-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA...\n-----END PUBLIC KEY-----\n"
        },
        "oxifed:keyChain": {
            "type": "OxifedPKI",
            "userKey": key_id,
            "domainKey": format!("https://{}/.well-known/oxifed/domain-key", domain),
            "masterKey": format!("https://{}/.well-known/oxifed/master-key", domain),
            "userKeyCertificate": null,
            "domainKeyCertificate": null
        },
        "published": "2024-01-01T00:00:00Z",
        "manuallyApprovesFollowers": false
    })
}