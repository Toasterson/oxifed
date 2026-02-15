use chrono::{DateTime, Utc};
use futures::StreamExt;
use k8s_openapi::ByteString;
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, OwnerReference};
use kube::api::{DynamicObject, Patch, PatchParams};
use kube::runtime::Controller;
use kube::{Api, Client};
use kube::{CustomResource, ResourceExt, runtime::controller::Action};
use mongodb::bson::doc;
use oxifed::database::{
    DatabaseManager, DomainDocument, DomainStatus as DbDomainStatus, KeyDocument, KeyStatus,
    KeyType, RegistrationMode,
};
use oxifed::pki::{KeyAlgorithm, KeyPair, TrustLevel};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::time::Duration;

/// Spec for the Domain CRD
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "oxifed.io", version = "v1alpha1", kind = "Domain", namespaced)]
#[kube(status = "DomainStatus")]
pub struct DomainSpec {
    pub hostname: String,
    pub description: Option<String>,
    pub admin_email: Option<String>,
}

/// Status for the Domain CRD
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct DomainStatus {
    pub initialized: bool,
    pub last_reconciled: Option<DateTime<Utc>>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),
    #[error("Database Error: {0}")]
    DatabaseError(String),
    #[error("PKI Error: {0}")]
    PkiError(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Gateway/TLS configuration loaded from environment
#[derive(Clone)]
struct GatewayConfig {
    gateway_name: String,
    gateway_namespace: String,
    cert_issuer_name: String,
    cert_issuer_kind: String,
    domainservd_service_name: String,
    domainservd_service_port: i64,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            gateway_name: "external".to_string(),
            gateway_namespace: "envoy-gateway-system".to_string(),
            cert_issuer_name: "letsencrypt".to_string(),
            cert_issuer_kind: "ClusterIssuer".to_string(),
            domainservd_service_name: "domainservd".to_string(),
            domainservd_service_port: 80,
        }
    }
}

impl GatewayConfig {
    fn from_env() -> Self {
        Self {
            gateway_name: std::env::var("GATEWAY_NAME").unwrap_or_else(|_| "external".to_string()),
            gateway_namespace: std::env::var("GATEWAY_NAMESPACE")
                .unwrap_or_else(|_| "envoy-gateway-system".to_string()),
            cert_issuer_name: std::env::var("CERT_ISSUER_NAME")
                .unwrap_or_else(|_| "letsencrypt".to_string()),
            cert_issuer_kind: std::env::var("CERT_ISSUER_KIND")
                .unwrap_or_else(|_| "ClusterIssuer".to_string()),
            domainservd_service_name: std::env::var("DOMAINSERVD_SERVICE_NAME")
                .unwrap_or_else(|_| "domainservd".to_string()),
            domainservd_service_port: std::env::var("DOMAINSERVD_SERVICE_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(80),
        }
    }
}

struct Context {
    client: Client,
    db_manager: Option<DatabaseManager>,
    gateway_config: GatewayConfig,
}

async fn reconcile(domain: Arc<Domain>, ctx: Arc<Context>) -> Result<Action> {
    if domain.metadata.deletion_timestamp.is_some() {
        return Ok(Action::await_change());
    }

    let ns = domain.namespace().unwrap();
    let domains: Api<Domain> = Api::namespaced(ctx.client.clone(), &ns);
    let secrets: Api<Secret> = Api::namespaced(ctx.client.clone(), &ns);

    tracing::info!("Reconciling Domain: {}", domain.name_any());

    // 1. Generate Ed25519 keys if not present
    let secret_name = format!("{}-keys", domain.name_any());

    match secrets
        .get_opt(&secret_name)
        .await
        .map_err(Error::KubeError)?
    {
        Some(secret) => {
            tracing::debug!("Secret {} already exists", secret_name);
            // Even if secret exists, ensure key is in MongoDB
            if let Some(ref db_manager) = ctx.db_manager
                && let Some(data) = secret.data
                && let (Some(pub_key), Some(priv_key)) =
                    (data.get("public_key.pem"), data.get("private_key.pem"))
            {
                let pub_key_str = String::from_utf8_lossy(&pub_key.0).to_string();
                let priv_key_str = String::from_utf8_lossy(&priv_key.0).to_string();

                // Calculate fingerprint from the public key
                let fingerprint = {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(pub_key_str.as_bytes());
                    let result = hasher.finalize();
                    format!("sha256:{}", hex::encode(result))
                };

                let key_doc = KeyDocument {
                    id: None,
                    key_id: secret_name.clone(),
                    actor_id: format!("https://{}/actor", domain.spec.hostname),
                    key_type: KeyType::Domain,
                    algorithm: "Ed25519".to_string(),
                    key_size: None,
                    public_key_pem: pub_key_str,
                    private_key_pem: Some(priv_key_str),
                    encryption_algorithm: None,
                    fingerprint,
                    trust_level: TrustLevel::MasterSigned,
                    domain_signature: None,
                    master_signature: None,
                    usage: vec!["signing".to_string()],
                    status: KeyStatus::Active,
                    created_at: Utc::now(),
                    expires_at: None,
                    rotation_policy: None,
                    domain: Some(domain.spec.hostname.clone()),
                };
                db_manager
                    .upsert_key(key_doc)
                    .await
                    .map_err(|e| Error::DatabaseError(e.to_string()))?;
            }
        }
        None => {
            tracing::info!("Generating keys for Domain: {}", domain.name_any());
            let key_pair = KeyPair::generate(KeyAlgorithm::Ed25519)
                .map_err(|e| Error::PkiError(e.to_string()))?;

            let pub_key_pem = key_pair.public_key.pem_data.clone();
            let priv_key_pem = key_pair.private_key.encrypted_pem.clone();
            let fingerprint = key_pair.public_key.fingerprint.clone();

            let mut data = BTreeMap::new();
            data.insert(
                "public_key.pem".to_string(),
                ByteString(pub_key_pem.as_bytes().to_vec()),
            );
            data.insert(
                "private_key.pem".to_string(),
                ByteString(priv_key_pem.as_bytes().to_vec()),
            );

            let secret = Secret {
                metadata: ObjectMeta {
                    name: Some(secret_name.clone()),
                    namespace: Some(ns.clone()),
                    ..Default::default()
                },
                data: Some(data),
                ..Default::default()
            };

            secrets
                .create(&kube::api::PostParams::default(), &secret)
                .await
                .map_err(Error::KubeError)?;

            if let Some(ref db_manager) = ctx.db_manager {
                let key_doc = KeyDocument {
                    id: None,
                    key_id: secret_name.clone(),
                    actor_id: format!("https://{}/actor", domain.spec.hostname),
                    key_type: KeyType::Domain,
                    algorithm: "Ed25519".to_string(),
                    key_size: None,
                    public_key_pem: pub_key_pem,
                    private_key_pem: Some(priv_key_pem),
                    encryption_algorithm: None,
                    fingerprint,
                    trust_level: TrustLevel::MasterSigned,
                    domain_signature: None,
                    master_signature: None,
                    usage: vec!["signing".to_string()],
                    status: KeyStatus::Active,
                    created_at: Utc::now(),
                    expires_at: None,
                    rotation_policy: None,
                    domain: Some(domain.spec.hostname.clone()),
                };
                db_manager
                    .upsert_key(key_doc)
                    .await
                    .map_err(|e| Error::DatabaseError(e.to_string()))?;
            }
        }
    }

    // 2. Ensure networking resources (Certificate, ReferenceGrant, HTTPRoute)
    let domain_resource_name = domain.name_any();
    let hostname = &domain.spec.hostname;
    let gw = &ctx.gateway_config;

    let owner_ref = OwnerReference {
        api_version: "oxifed.io/v1alpha1".to_string(),
        kind: "Domain".to_string(),
        name: domain.name_any(),
        uid: domain.metadata.uid.clone().unwrap_or_default(),
        controller: Some(true),
        block_owner_deletion: Some(true),
    };

    // 2a. cert-manager Certificate
    ensure_certificate(
        &ctx.client,
        &ns,
        &domain_resource_name,
        hostname,
        gw,
        &owner_ref,
    )
    .await?;

    // 2b. ReferenceGrant (allows Gateway namespace to reference our cert secret)
    ensure_reference_grant(&ctx.client, &ns, &domain_resource_name, gw, &owner_ref).await?;

    // 2c. HTTPRoute
    ensure_httproute(
        &ctx.client,
        &ns,
        &domain_resource_name,
        hostname,
        gw,
        &owner_ref,
    )
    .await?;

    // 3. Update MongoDB with the domain configuration
    if let Some(ref db_manager) = ctx.db_manager {
        tracing::info!("Updating MongoDB for Domain: {}", domain.name_any());

        let db_domain = DomainDocument {
            id: None,
            domain: domain.spec.hostname.clone(),
            name: Some(domain.name_any()),
            description: domain.spec.description.clone(),
            contact_email: domain.spec.admin_email.clone(),
            rules: None,
            registration_mode: RegistrationMode::Closed,
            authorized_fetch: true,
            max_note_length: Some(500),
            max_file_size: Some(10 * 1024 * 1024),
            allowed_file_types: Some(vec!["image/jpeg".to_string(), "image/png".to_string()]),
            domain_key_id: Some(secret_name),
            config: None,
            status: DbDomainStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        db_manager
            .upsert_domain(db_domain)
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
    } else {
        tracing::warn!("MongoDB manager not initialized, skipping database update");
    }

    // 3. Update the status
    let new_status = DomainStatus {
        initialized: true,
        last_reconciled: Some(Utc::now()),
    };

    let patch = serde_json::json!({
        "status": new_status
    });

    match domains
        .patch_status(
            &domain.name_any(),
            &kube::api::PatchParams::default(),
            &kube::api::Patch::Merge(&patch),
        )
        .await
    {
        Ok(_) => Ok(Action::requeue(Duration::from_secs(3600))),
        Err(kube::Error::Api(e)) if e.code == 404 => {
            tracing::warn!(
                "Domain {} not found during status patch, it might have been deleted",
                domain.name_any()
            );
            Ok(Action::await_change())
        }
        Err(e) => Err(Error::KubeError(e)),
    }
}

/// Ensure a cert-manager Certificate exists for the domain
async fn ensure_certificate(
    client: &Client,
    ns: &str,
    domain_resource_name: &str,
    hostname: &str,
    gw: &GatewayConfig,
    owner_ref: &OwnerReference,
) -> Result<()> {
    let cert_name = format!("{}-tls", domain_resource_name);
    let cert_json = serde_json::json!({
        "apiVersion": "cert-manager.io/v1",
        "kind": "Certificate",
        "metadata": {
            "name": cert_name,
            "namespace": ns,
            "ownerReferences": [owner_ref],
        },
        "spec": {
            "secretName": cert_name,
            "commonName": hostname,
            "dnsNames": [hostname],
            "issuerRef": {
                "name": gw.cert_issuer_name,
                "kind": gw.cert_issuer_kind,
            }
        }
    });

    let api_resource = kube::api::ApiResource {
        group: "cert-manager.io".to_string(),
        version: "v1".to_string(),
        api_version: "cert-manager.io/v1".to_string(),
        kind: "Certificate".to_string(),
        plural: "certificates".to_string(),
    };
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), ns, &api_resource);

    let obj: DynamicObject = serde_json::from_value(cert_json)
        .map_err(|e| Error::DatabaseError(format!("Failed to build Certificate JSON: {}", e)))?;

    api.patch(
        &cert_name,
        &PatchParams::apply("oxifed-operator").force(),
        &Patch::Apply(&obj),
    )
    .await
    .map_err(Error::KubeError)?;

    tracing::info!("Ensured Certificate: {}", cert_name);
    Ok(())
}

/// Ensure a ReferenceGrant exists allowing the Gateway namespace to reference our cert secret
async fn ensure_reference_grant(
    client: &Client,
    ns: &str,
    domain_resource_name: &str,
    gw: &GatewayConfig,
    owner_ref: &OwnerReference,
) -> Result<()> {
    let grant_name = format!("{}-tls-grant", domain_resource_name);
    let secret_name = format!("{}-tls", domain_resource_name);
    let grant_json = serde_json::json!({
        "apiVersion": "gateway.networking.k8s.io/v1beta1",
        "kind": "ReferenceGrant",
        "metadata": {
            "name": grant_name,
            "namespace": ns,
            "ownerReferences": [owner_ref],
        },
        "spec": {
            "from": [{
                "group": "gateway.networking.k8s.io",
                "kind": "Gateway",
                "namespace": gw.gateway_namespace,
            }],
            "to": [{
                "group": "",
                "kind": "Secret",
                "name": secret_name,
            }]
        }
    });

    let api_resource = kube::api::ApiResource {
        group: "gateway.networking.k8s.io".to_string(),
        version: "v1beta1".to_string(),
        api_version: "gateway.networking.k8s.io/v1beta1".to_string(),
        kind: "ReferenceGrant".to_string(),
        plural: "referencegrants".to_string(),
    };
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), ns, &api_resource);

    let obj: DynamicObject = serde_json::from_value(grant_json)
        .map_err(|e| Error::DatabaseError(format!("Failed to build ReferenceGrant JSON: {}", e)))?;

    api.patch(
        &grant_name,
        &PatchParams::apply("oxifed-operator").force(),
        &Patch::Apply(&obj),
    )
    .await
    .map_err(Error::KubeError)?;

    tracing::info!("Ensured ReferenceGrant: {}", grant_name);
    Ok(())
}

/// Ensure an HTTPRoute exists routing traffic for this domain to domainservd
async fn ensure_httproute(
    client: &Client,
    ns: &str,
    domain_resource_name: &str,
    hostname: &str,
    gw: &GatewayConfig,
    owner_ref: &OwnerReference,
) -> Result<()> {
    let route_name = domain_resource_name.to_string();
    let route_json = serde_json::json!({
        "apiVersion": "gateway.networking.k8s.io/v1",
        "kind": "HTTPRoute",
        "metadata": {
            "name": route_name,
            "namespace": ns,
            "ownerReferences": [owner_ref],
        },
        "spec": {
            "parentRefs": [{
                "name": gw.gateway_name,
                "namespace": gw.gateway_namespace,
            }],
            "hostnames": [hostname],
            "rules": [{
                "matches": [{
                    "path": {
                        "type": "PathPrefix",
                        "value": "/"
                    }
                }],
                "backendRefs": [{
                    "name": gw.domainservd_service_name,
                    "port": gw.domainservd_service_port,
                }]
            }]
        }
    });

    let api_resource = kube::api::ApiResource {
        group: "gateway.networking.k8s.io".to_string(),
        version: "v1".to_string(),
        api_version: "gateway.networking.k8s.io/v1".to_string(),
        kind: "HTTPRoute".to_string(),
        plural: "httproutes".to_string(),
    };
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), ns, &api_resource);

    let obj: DynamicObject = serde_json::from_value(route_json)
        .map_err(|e| Error::DatabaseError(format!("Failed to build HTTPRoute JSON: {}", e)))?;

    api.patch(
        &route_name,
        &PatchParams::apply("oxifed-operator").force(),
        &Patch::Apply(&obj),
    )
    .await
    .map_err(Error::KubeError)?;

    tracing::info!("Ensured HTTPRoute: {}", route_name);
    Ok(())
}

fn error_policy(_domain: Arc<Domain>, error: &Error, _ctx: Arc<Context>) -> Action {
    tracing::error!("Reconciliation error: {:?}", error);
    Action::requeue(Duration::from_secs(60))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let client = Client::try_default().await.map_err(Error::KubeError)?;
    let domains: Api<Domain> = Api::all(client.clone());

    let mongodb_uri = std::env::var("MONGODB_URI").ok();
    let db_manager = if let Some(uri) = mongodb_uri {
        tracing::info!("Connecting to MongoDB");
        let client_options = mongodb::options::ClientOptions::parse(&uri)
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
        let mongo_client = mongodb::Client::with_options(client_options)
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
        let db_name = std::env::var("MONGODB_DBNAME").unwrap_or_else(|_| "domainservd".to_string());
        let database = mongo_client.database(&db_name);
        let manager = DatabaseManager::new(database);
        manager
            .initialize()
            .await
            .map_err(|e| Error::DatabaseError(e.to_string()))?;
        Some(manager)
    } else {
        tracing::warn!("MONGODB_URI not set, operator will run without database integration");
        None
    };

    let gateway_config = GatewayConfig::from_env();
    tracing::info!(
        "Gateway config: {} in namespace {}",
        gateway_config.gateway_name,
        gateway_config.gateway_namespace
    );

    let context = Arc::new(Context {
        client: client.clone(),
        db_manager,
        gateway_config,
    });

    tracing::info!("Starting Domain Operator");

    Controller::new(domains, kube::runtime::watcher::Config::default())
        .run(reconcile, error_policy, context)
        .for_each(|res| async move {
            match res {
                Ok(o) => tracing::info!("Reconciled {:?}", o),
                Err(e) => tracing::error!("Reconcile failed: {:?}", e),
            }
        })
        .await;

    Ok(())
}
