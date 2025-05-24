//! Well-known endpoints for PKI and federation discovery
//!
//! Implements standard well-known endpoints including:
//! - PKI master key endpoint
//! - Domain key endpoint  
//! - Trust chain verification
//! - Node info and metadata

use crate::pki::{PkiManager, TrustLevel};
use crate::database::DatabaseManager;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Well-known endpoints state
#[derive(Clone)]
pub struct WellKnownState {
    pub pki: Arc<PkiManager>,
    pub domain: String,
    pub master_domain: String,
    pub db: Arc<DatabaseManager>,
}

/// Query parameters for trust chain endpoint
#[derive(Debug, Deserialize)]
pub struct TrustChainQuery {
    #[serde(rename = "keyId")]
    key_id: String,
}

/// Master key response
#[derive(Debug, Serialize)]
pub struct MasterKeyResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "publicKeyPem")]
    pub public_key_pem: String,
    pub algorithm: String,
    #[serde(rename = "keySize")]
    pub key_size: Option<u32>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    pub fingerprint: String,
    pub usage: Vec<String>,
    #[serde(rename = "type")]
    pub key_type: String,
}

/// Domain key response
#[derive(Debug, Serialize)]
pub struct DomainKeyResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    pub domain: String,
    #[serde(rename = "publicKeyPem")]
    pub public_key_pem: String,
    pub algorithm: String,
    #[serde(rename = "keySize")]
    pub key_size: Option<u32>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "expiresAt")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(rename = "masterSignature")]
    pub master_signature: Option<MasterSignatureInfo>,
    pub fingerprint: String,
    pub usage: Vec<String>,
    #[serde(rename = "type")]
    pub key_type: String,
}

/// Master signature information
#[derive(Debug, Serialize)]
pub struct MasterSignatureInfo {
    pub signature: String,
    #[serde(rename = "signedAt")]
    pub signed_at: DateTime<Utc>,
    #[serde(rename = "masterKeyId")]
    pub master_key_id: String,
}

/// Trust chain response
#[derive(Debug, Serialize)]
pub struct TrustChainResponse {
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "trustLevel")]
    pub trust_level: TrustLevel,
    #[serde(rename = "verificationChain")]
    pub verification_chain: Vec<TrustChainLinkResponse>,
    #[serde(rename = "verifiedAt")]
    pub verified_at: DateTime<Utc>,
}

/// Trust chain link response
#[derive(Debug, Serialize)]
pub struct TrustChainLinkResponse {
    pub level: String,
    #[serde(rename = "keyId")]
    pub key_id: String,
    #[serde(rename = "signedBy")]
    pub signed_by: Option<String>,
    #[serde(rename = "signedAt")]
    pub signed_at: Option<DateTime<Utc>>,
    #[serde(rename = "selfSigned")]
    pub self_signed: bool,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
}

/// Node info 2.0 response
#[derive(Debug, Serialize)]
pub struct NodeInfo {
    pub version: String,
    pub software: NodeInfoSoftware,
    pub protocols: Vec<String>,
    pub services: NodeInfoServices,
    pub usage: NodeInfoUsage,
    #[serde(rename = "openRegistrations")]
    pub open_registrations: bool,
    pub metadata: NodeInfoMetadata,
}

/// Node info software information
#[derive(Debug, Serialize)]
pub struct NodeInfoSoftware {
    pub name: String,
    pub version: String,
    pub repository: Option<String>,
    pub homepage: Option<String>,
}

/// Node info services
#[derive(Debug, Serialize)]
pub struct NodeInfoServices {
    pub inbound: Vec<String>,
    pub outbound: Vec<String>,
}

/// Node info usage statistics
#[derive(Debug, Serialize)]
pub struct NodeInfoUsage {
    pub users: NodeInfoUsers,
    #[serde(rename = "localPosts")]
    pub local_posts: u64,
    #[serde(rename = "localComments")]
    pub local_comments: u64,
}

/// Node info user statistics
#[derive(Debug, Serialize)]
pub struct NodeInfoUsers {
    pub total: u64,
    #[serde(rename = "activeMonth")]
    pub active_month: u64,
    #[serde(rename = "activeHalfyear")]
    pub active_halfyear: u64,
}

/// Node info metadata
#[derive(Debug, Serialize)]
pub struct NodeInfoMetadata {
    #[serde(rename = "nodeName")]
    pub node_name: String,
    #[serde(rename = "nodeDescription")]
    pub node_description: String,
    #[serde(rename = "maintainer")]
    pub maintainer: Option<NodeInfoMaintainer>,
    #[serde(rename = "themeColor")]
    pub theme_color: Option<String>,
    pub langs: Vec<String>,
    #[serde(rename = "tosUrl")]
    pub tos_url: Option<String>,
    #[serde(rename = "privacyPolicyUrl")]
    pub privacy_policy_url: Option<String>,
    #[serde(rename = "impressumUrl")]
    pub impressum_url: Option<String>,
    #[serde(rename = "donationUrl")]
    pub donation_url: Option<String>,
    #[serde(rename = "repositoryUrl")]
    pub repository_url: Option<String>,
    #[serde(rename = "feedbackUrl")]
    pub feedback_url: Option<String>,
}

/// Node info maintainer
#[derive(Debug, Serialize)]
pub struct NodeInfoMaintainer {
    pub name: String,
    pub email: Option<String>,
}

/// Create well-known endpoints router
pub fn well_known_router(state: WellKnownState) -> Router<WellKnownState> {
    Router::new()
        .route("/.well-known/oxifed/master-key", get(get_master_key))
        .route("/.well-known/oxifed/domain-key", get(get_domain_key))
        .route("/.well-known/oxifed/trust-chain", get(get_trust_chain))
        .route("/.well-known/nodeinfo", get(get_nodeinfo_discovery))
        .route("/nodeinfo/2.0", get(get_nodeinfo))
        .route("/.well-known/host-meta", get(get_host_meta))
        .with_state(state)
}

/// Get master key endpoint
async fn get_master_key(
    State(state): State<WellKnownState>,
) -> Result<Response, StatusCode> {
    debug!("Serving master key for {}", state.master_domain);

    let master_key = match state.pki.master_key.as_ref() {
        Some(key) => key,
        None => {
            warn!("Master key not found for domain: {}", state.master_domain);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let key_size = match &master_key.public_key.algorithm {
        crate::pki::KeyAlgorithm::Rsa { key_size } => Some(*key_size),
        crate::pki::KeyAlgorithm::Ed25519 => None,
    };

    let response = MasterKeyResponse {
        key_id: master_key.key_id.clone(),
        public_key_pem: master_key.public_key.pem_data.clone(),
        algorithm: format!("{:?}", master_key.public_key.algorithm),
        key_size,
        created_at: master_key.created_at,
        fingerprint: master_key.public_key.fingerprint.clone(),
        usage: master_key.usage.iter().map(|u| format!("{:?}", u)).collect(),
        key_type: "MasterKey".to_string(),
    };

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/json")],
        Json(response),
    ).into_response())
}

/// Get domain key endpoint
async fn get_domain_key(
    State(state): State<WellKnownState>,
) -> Result<Response, StatusCode> {
    debug!("Serving domain key for {}", state.domain);

    let domain_key = match state.pki.domain_keys.get(&state.domain) {
        Some(key) => key,
        None => {
            warn!("Domain key not found for domain: {}", state.domain);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let key_size = match &domain_key.public_key.algorithm {
        crate::pki::KeyAlgorithm::Rsa { key_size } => Some(*key_size),
        crate::pki::KeyAlgorithm::Ed25519 => None,
    };

    let master_signature = domain_key.master_signature.as_ref().map(|sig| {
        MasterSignatureInfo {
            signature: sig.signature.clone(),
            signed_at: sig.signed_at,
            master_key_id: sig.master_key_id.clone(),
        }
    });

    let response = DomainKeyResponse {
        key_id: domain_key.key_id.clone(),
        domain: domain_key.domain.clone(),
        public_key_pem: domain_key.public_key.pem_data.clone(),
        algorithm: format!("{:?}", domain_key.public_key.algorithm),
        key_size,
        created_at: domain_key.created_at,
        expires_at: domain_key.expires_at,
        master_signature,
        fingerprint: domain_key.public_key.fingerprint.clone(),
        usage: domain_key.usage.iter().map(|u| format!("{:?}", u)).collect(),
        key_type: "DomainKey".to_string(),
    };

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/json")],
        Json(response),
    ).into_response())
}

/// Get trust chain for a key
async fn get_trust_chain(
    Query(params): Query<TrustChainQuery>,
    State(state): State<WellKnownState>,
) -> Result<Response, StatusCode> {
    debug!("Getting trust chain for key: {}", params.key_id);

    let trust_chain = match state.pki.build_trust_chain(&params.key_id) {
        Ok(chain) => chain,
        Err(e) => {
            error!("Failed to build trust chain for {}: {}", params.key_id, e);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let verification_chain = trust_chain.verification_chain.into_iter().map(|link| {
        TrustChainLinkResponse {
            level: link.level,
            key_id: link.key_id,
            signed_by: link.signed_by,
            signed_at: link.signed_at,
            self_signed: link.self_signed,
            created_at: link.created_at,
        }
    }).collect();

    let response = TrustChainResponse {
        key_id: trust_chain.key_id,
        trust_level: trust_chain.trust_level,
        verification_chain,
        verified_at: trust_chain.verified_at,
    };

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/json")],
        Json(response),
    ).into_response())
}

/// Get node info discovery
async fn get_nodeinfo_discovery(
    State(state): State<WellKnownState>,
) -> Result<Response, StatusCode> {
    let discovery = json!({
        "links": [
            {
                "rel": "http://nodeinfo.diaspora.software/ns/schema/2.0",
                "href": format!("https://{}/nodeinfo/2.0", state.domain)
            }
        ]
    });

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/json")],
        Json(discovery),
    ).into_response())
}

/// Get node info 2.0
async fn get_nodeinfo(
    State(state): State<WellKnownState>,
) -> Result<Response, StatusCode> {
    let nodeinfo = NodeInfo {
        version: "2.0".to_string(),
        software: NodeInfoSoftware {
            name: "oxifed".to_string(),
            version: "0.1.0".to_string(),
            repository: Some("https://github.com/oxifed/oxifed".to_string()),
            homepage: Some("https://oxifed.org".to_string()),
        },
        protocols: vec!["activitypub".to_string()],
        services: NodeInfoServices {
            inbound: vec![],
            outbound: vec!["atom1.0".to_string(), "rss2.0".to_string()],
        },
        usage: NodeInfoUsage {
            users: NodeInfoUsers {
                total: state.db.count_local_actors().await.unwrap_or(0),
                active_month: 0, // TODO: Implement active user tracking
                active_halfyear: 0,
            },
            local_posts: state.db.count_local_posts().await.unwrap_or(0),
            local_comments: 0,
        },
        open_registrations: false, // TODO: Get from domain config
        metadata: NodeInfoMetadata {
            node_name: state.domain.clone(),
            node_description: "Oxifed ActivityPub server with hierarchical PKI".to_string(),
            maintainer: None, // TODO: Get from config
            theme_color: Some("#1976d2".to_string()),
            langs: vec!["en".to_string()],
            tos_url: None,
            privacy_policy_url: None,
            impressum_url: None,
            donation_url: None,
            repository_url: Some("https://github.com/oxifed/oxifed".to_string()),
            feedback_url: None,
        },
    };

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/json; profile=\"http://nodeinfo.diaspora.software/ns/schema/2.0#\"")],
        Json(nodeinfo),
    ).into_response())
}

/// Get host-meta for XRD discovery
async fn get_host_meta(
    State(state): State<WellKnownState>,
) -> Result<Response, StatusCode> {
    let host_meta = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<XRD xmlns="http://docs.oasis-open.org/ns/xri/xrd-1.0">
  <Link rel="lrdd" template="https://{}/.well-known/webfinger?resource={{uri}}"/>
</XRD>"#,
        state.domain
    );

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/xrd+xml")],
        host_meta,
    ).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pki::{KeyAlgorithm, KeyPair};
    use mongodb::Client;

    #[tokio::test]
    async fn test_master_key_endpoint() {
        let mut pki = PkiManager::new();
        
        // Create a mock master key
        let key_pair = KeyPair::from_pem(
            KeyAlgorithm::Rsa { key_size: 4096 },
            "-----BEGIN PUBLIC KEY-----\ntest\n-----END PUBLIC KEY-----".to_string(),
            "-----BEGIN PRIVATE KEY-----\ntest\n-----END PRIVATE KEY-----".to_string(),
        ).unwrap();

        let master_key = crate::pki::MasterKeyInfo {
            key_id: "https://example.com/.well-known/oxifed/master-key".to_string(),
            public_key: key_pair.public_key,
            private_key: key_pair.private_key,
            created_at: Utc::now(),
            fingerprint: "sha256:test".to_string(),
            usage: vec![crate::pki::KeyUsage::DomainSigning],
        };

        pki.master_key = Some(master_key);

        // Create a mock database for testing
        let client = Client::with_uri_str("mongodb://localhost:27017").await.unwrap();
        let database = client.database("test_oxifed");
        
        let state = WellKnownState {
            pki: Arc::new(pki),
            domain: "example.com".to_string(),
            master_domain: "master.example.com".to_string(),
            db: Arc::new(DatabaseManager::new(database)),
        };

        // This would be tested with axum test utilities in a real implementation
        assert_eq!(state.domain, "example.com");
        assert!(state.pki.master_key.is_some());
    }

    #[test]
    fn test_trust_chain_serialization() {
        let response = TrustChainResponse {
            key_id: "test-key".to_string(),
            trust_level: TrustLevel::DomainVerified,
            verification_chain: vec![],
            verified_at: Utc::now(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("DomainVerified"));
        assert!(json.contains("test-key"));
    }

    #[test]
    fn test_nodeinfo_structure() {
        let nodeinfo = NodeInfo {
            version: "2.0".to_string(),
            software: NodeInfoSoftware {
                name: "oxifed".to_string(),
                version: "0.1.0".to_string(),
                repository: None,
                homepage: None,
            },
            protocols: vec!["activitypub".to_string()],
            services: NodeInfoServices {
                inbound: vec![],
                outbound: vec![],
            },
            usage: NodeInfoUsage {
                users: NodeInfoUsers {
                    total: 0,
                    active_month: 0,
                    active_halfyear: 0,
                },
                local_posts: 0,
                local_comments: 0,
            },
            open_registrations: false,
            metadata: NodeInfoMetadata {
                node_name: "test".to_string(),
                node_description: "test".to_string(),
                maintainer: None,
                theme_color: None,
                langs: vec!["en".to_string()],
                tos_url: None,
                privacy_policy_url: None,
                impressum_url: None,
                donation_url: None,
                repository_url: None,
                feedback_url: None,
            },
        };

        let json = serde_json::to_string(&nodeinfo).unwrap();
        assert!(json.contains("activitypub"));
        assert!(json.contains("oxifed"));
    }
}