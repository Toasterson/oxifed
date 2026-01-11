use chrono::{DateTime, Utc};
use futures::StreamExt;
use k8s_openapi::ByteString;
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
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

struct Context {
    client: Client,
    db_manager: Option<DatabaseManager>,
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
            if let Some(ref db_manager) = ctx.db_manager {
                if let Some(data) = secret.data {
                    if let (Some(pub_key), Some(_priv_key)) =
                        (data.get("public_key.pem"), data.get("private_key.pem"))
                    {
                        let pub_key_str = String::from_utf8_lossy(&pub_key.0).to_string();
                        let priv_key_str = String::from_utf8_lossy(&_priv_key.0).to_string();

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
                            fingerprint: "MOCK_FINGERPRINT".to_string(),
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
        }
        None => {
            tracing::info!("Generating keys for Domain: {}", domain.name_any());
            let _key_pair = KeyPair::generate(KeyAlgorithm::Ed25519)
                .map_err(|e| Error::PkiError(e.to_string()))?;

            let pub_key_pem = "MOCK_PUBLIC_KEY".to_string();
            let priv_key_pem = "MOCK_PRIVATE_KEY".to_string();

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
                    fingerprint: "MOCK_FINGERPRINT".to_string(),
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

    // 2. Update MongoDB with the domain configuration
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

    let context = Arc::new(Context {
        client: client.clone(),
        db_manager,
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
