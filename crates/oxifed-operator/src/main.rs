use chrono::{DateTime, Utc};
use futures::StreamExt;
use kube::runtime::Controller;
use kube::{Api, Client};
use kube::{CustomResource, ResourceExt, runtime::controller::Action};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
}

pub type Result<T> = std::result::Result<T, Error>;

struct Context {
    client: Client,
}

async fn reconcile(domain: Arc<Domain>, ctx: Arc<Context>) -> Result<Action> {
    if domain.metadata.deletion_timestamp.is_some() {
        return Ok(Action::await_change());
    }

    let ns = domain.namespace().unwrap();
    let domains: Api<Domain> = Api::namespaced(ctx.client.clone(), &ns);

    tracing::info!("Reconciling Domain: {}", domain.name_any());

    // In a real implementation, this would:
    // 1. Generate Ed25519 keys if not present
    // 2. Create a Kubernetes Secret with the keys
    // 3. Update MongoDB with the domain configuration

    // For now, we just update the status
    let new_status = DomainStatus {
        initialized: true,
        last_reconciled: Some(Utc::now()),
    };

    let patch = serde_json::json!({
        "apiVersion": "oxifed.io/v1alpha1",
        "kind": "Domain",
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

    let context = Arc::new(Context {
        client: client.clone(),
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
