use std::sync::Arc;

use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_protocol::{QualifiedAddress, ROOT_AGENT_NAME};
use loopal_storage::SessionStore;
use tokio::sync::{Mutex, mpsc};

use super::*;

fn settings() -> WorkflowSettings {
    WorkflowSettings {
        policy: OrchestrationPolicy::Explicit,
        execution_enabled: true,
        ..WorkflowSettings::default()
    }
}

fn hub() -> Arc<Mutex<Hub>> {
    let (events, _receiver) = mpsc::channel(16);
    let mut hub = Hub::new(events);
    hub.set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    Arc::new(Mutex::new(hub))
}

fn owner() -> WorkflowOwner {
    WorkflowOwner::new(
        "session-admission",
        QualifiedAddress::local(ROOT_AGENT_NAME),
    )
}

async fn runtime(hub: Arc<Mutex<Hub>>, temp: &tempfile::TempDir) -> WorkflowRuntime {
    let sessions = Arc::new(SessionStore::with_base_dir(temp.path().to_path_buf()));
    WorkflowRuntime::new_production(hub, sessions, &settings())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn terminal_activation_requires_completed_admission() {
    let temp = tempfile::tempdir().unwrap();
    let runtime = runtime(hub(), &temp).await;
    assert!(matches!(
        runtime.activate_terminal_deliveries().await,
        Err(WorkflowRuntimeError::Coordinator(
            WorkflowCoordinatorError::RecoveryRequired
        ))
    ));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn admitted_runtime_activates_once_and_clear_admission_is_idempotent() {
    let temp = tempfile::tempdir().unwrap();
    let hub = hub();
    let mut runtime = runtime(hub.clone(), &temp).await;
    assert_eq!(runtime.recover_and_admit(owner()).await.unwrap(), 0);
    assert!(matches!(
        runtime.recover_and_admit(owner()).await,
        Err(WorkflowRuntimeError::AlreadyAdmitted)
    ));
    runtime.activate_terminal_deliveries().await.unwrap();

    runtime.clear_admission().await;
    runtime.clear_admission().await;
    assert!(hub.lock().await.workflow_coordinator().is_none());
    runtime.shutdown().await.unwrap();
}
