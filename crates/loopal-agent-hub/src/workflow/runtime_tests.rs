use std::sync::Arc;

use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_protocol::{QualifiedAddress, ROOT_AGENT_NAME};
use loopal_storage::SessionStore;
use tokio::sync::{Mutex, mpsc};

use super::*;
use crate::workflow::WorkflowCoordinator;

fn enabled_settings() -> WorkflowSettings {
    WorkflowSettings {
        policy: OrchestrationPolicy::Explicit,
        execution_enabled: true,
        ..WorkflowSettings::default()
    }
}

fn test_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<loopal_protocol::AgentEvent>) {
    let (events, receiver) = mpsc::channel(16);
    let mut hub = Hub::new(events);
    hub.set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    (Arc::new(Mutex::new(hub)), receiver)
}

fn sessions(temp: &tempfile::TempDir) -> Arc<SessionStore> {
    Arc::new(SessionStore::with_base_dir(temp.path().to_path_buf()))
}

fn owner() -> WorkflowOwner {
    WorkflowOwner::new("session-runtime", QualifiedAddress::local(ROOT_AGENT_NAME))
}

#[tokio::test]
async fn factory_uses_exact_tool_enablement_predicate() {
    for settings in [
        WorkflowSettings::default(),
        WorkflowSettings {
            execution_enabled: true,
            ..WorkflowSettings::default()
        },
        WorkflowSettings {
            policy: OrchestrationPolicy::Explicit,
            ..WorkflowSettings::default()
        },
    ] {
        let temp = tempfile::tempdir().unwrap();
        let (hub, _events) = test_hub();
        assert!(
            WorkflowRuntime::new_production(hub, sessions(&temp), &settings)
                .await
                .unwrap()
                .is_none()
        );
    }

    let temp = tempfile::tempdir().unwrap();
    let (hub, _events) = test_hub();
    let runtime = WorkflowRuntime::new_production(hub, sessions(&temp), &enabled_settings())
        .await
        .unwrap()
        .expect("enabled settings must construct the production runtime");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn enabled_runtime_requires_protected_audit() {
    let temp = tempfile::tempdir().unwrap();
    let (events, _receiver) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::new(events)));

    assert!(matches!(
        WorkflowRuntime::new_production(hub, sessions(&temp), &enabled_settings()).await,
        Err(WorkflowRuntimeError::ProtectedAuditUnavailable)
    ));
}

#[tokio::test]
async fn recovery_precedes_admission_and_shutdown_joins_owned_tasks() {
    let temp = tempfile::tempdir().unwrap();
    let (hub, _events) = test_hub();
    let mut runtime =
        WorkflowRuntime::new_production(hub.clone(), sessions(&temp), &enabled_settings())
            .await
            .unwrap()
            .unwrap();

    assert!(hub.lock().await.workflow_coordinator().is_none());
    assert_eq!(runtime.recover_and_admit(owner()).await.unwrap(), 0);
    assert!(runtime.ticker.is_some());
    assert!(hub.lock().await.workflow_coordinator().is_some());

    runtime.shutdown().await.unwrap();
    assert!(hub.lock().await.workflow_coordinator().is_none());
}

#[tokio::test]
async fn failed_recovery_never_opens_admission_or_starts_ticker() {
    let temp = tempfile::tempdir().unwrap();
    let sessions = sessions(&temp);
    let path = sessions
        .workflow_journal_path("session-runtime", "wrun_corrupt")
        .unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"not json\n").unwrap();
    let (hub, _events) = test_hub();
    let mut runtime = WorkflowRuntime::new_production(hub.clone(), sessions, &enabled_settings())
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(
        runtime.recover_and_admit(owner()).await,
        Err(WorkflowRuntimeError::Coordinator(
            WorkflowCoordinatorError::RecoveryInvalid
        ))
    ));
    assert!(runtime.ticker.is_none());
    assert!(hub.lock().await.workflow_coordinator().is_none());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn occupied_admission_is_not_replaced() {
    let temp = tempfile::tempdir().unwrap();
    let (hub, _events) = test_hub();
    let (existing, existing_task) = WorkflowCoordinator::spawn_disabled();
    hub.lock()
        .await
        .install_workflow_coordinator(existing.clone());
    let mut runtime =
        WorkflowRuntime::new_production(hub.clone(), sessions(&temp), &enabled_settings())
            .await
            .unwrap()
            .unwrap();

    assert!(matches!(
        runtime.recover_and_admit(owner()).await,
        Err(WorkflowRuntimeError::AdmissionOccupied)
    ));
    assert!(runtime.ticker.is_none());
    assert!(
        hub.lock()
            .await
            .workflow_coordinator()
            .is_some_and(|current| current.same_channel(&existing))
    );

    runtime.shutdown().await.unwrap();
    hub.lock().await.clear_workflow_coordinator();
    existing.shutdown().await.unwrap();
    existing_task.await.unwrap();
}

#[tokio::test]
async fn shutdown_does_not_clear_a_replacement_coordinator() {
    let temp = tempfile::tempdir().unwrap();
    let (hub, _events) = test_hub();
    let mut runtime =
        WorkflowRuntime::new_production(hub.clone(), sessions(&temp), &enabled_settings())
            .await
            .unwrap()
            .unwrap();
    runtime.recover_and_admit(owner()).await.unwrap();

    let (replacement, replacement_task) = WorkflowCoordinator::spawn_disabled();
    hub.lock()
        .await
        .install_workflow_coordinator(replacement.clone());
    runtime.shutdown().await.unwrap();
    assert!(
        hub.lock()
            .await
            .workflow_coordinator()
            .is_some_and(|current| current.same_channel(&replacement))
    );

    hub.lock().await.clear_workflow_coordinator();
    replacement.shutdown().await.unwrap();
    replacement_task.await.unwrap();
}
