use std::sync::Arc;
use std::time::Duration;

use loopal_config::{OrchestrationPolicy, WorkflowSettings};
use loopal_protocol::{QualifiedAddress, ROOT_AGENT_NAME};
use loopal_storage::SessionStore;
use tokio::sync::{Mutex, mpsc, oneshot};

use super::cleanup::DropCleanupOutcome;
use super::*;

#[path = "runtime_cleanup_ticker_tests.rs"]
mod cleanup_ticker_tests;

fn test_hub() -> Arc<Mutex<Hub>> {
    let (events, _receiver) = mpsc::channel(16);
    let mut hub = Hub::new(events);
    hub.set_protected_audit(Arc::new(loopal_vault_api::NoopAuditSink));
    Arc::new(Mutex::new(hub))
}

fn settings() -> WorkflowSettings {
    WorkflowSettings {
        policy: OrchestrationPolicy::Explicit,
        execution_enabled: true,
        ..WorkflowSettings::default()
    }
}

fn owner() -> WorkflowOwner {
    WorkflowOwner::new("session-drop", QualifiedAddress::local(ROOT_AGENT_NAME))
}

#[tokio::test]
async fn drop_waits_for_contended_hub_and_cleans_exact_admission() {
    let temp = tempfile::tempdir().unwrap();
    let hub = test_hub();
    let sessions = Arc::new(SessionStore::with_base_dir(temp.path().to_path_buf()));
    let mut runtime = WorkflowRuntime::new_production(hub.clone(), sessions, &settings())
        .await
        .unwrap()
        .unwrap();
    runtime.recover_and_admit(owner()).await.unwrap();
    let handle = runtime.handle.clone();
    let shutdown_signal = runtime.shutdown_signal.clone();
    let (probe, cleanup) = oneshot::channel();
    runtime.drop_cleanup_probe = Some(probe);

    let guard = hub.clone().lock_owned().await;
    let mut shutdown = Box::pin(shutdown_signal.notified());
    drop(runtime);
    tokio::task::yield_now().await;
    assert!(
        guard
            .workflow_coordinator()
            .is_some_and(|current| current.same_channel(&handle))
    );
    drop(guard);

    let mut cleanup = Box::pin(cleanup);
    tokio::select! {
        _ = &mut shutdown => panic!("graceful drop must not request Hub shutdown"),
        outcome = &mut cleanup => {
            assert_eq!(outcome.unwrap(), DropCleanupOutcome::Graceful);
        }
    }
    assert!(hub.lock().await.workflow_coordinator().is_none());
    assert_eq!(
        handle.shutdown().await,
        Err(WorkflowCoordinatorError::Unavailable)
    );
}

#[tokio::test(start_paused = true)]
async fn drop_timeout_aborts_actor_and_requests_hub_shutdown() {
    let hub = test_hub();
    let shutdown_signal = hub.lock().await.shutdown_signal.clone();
    let (handle, actor_task, command_seen) = WorkflowCoordinatorHandle::spawn_test_blocked();
    hub.lock()
        .await
        .install_workflow_coordinator(handle.clone());
    let (probe, cleanup) = oneshot::channel();
    let timeout = Duration::from_secs(5);
    let runtime = WorkflowRuntime {
        hub: hub.clone(),
        shutdown_signal: shutdown_signal.clone(),
        handle: handle.clone(),
        actor_task: Some(actor_task),
        ticker: None,
        admitted: true,
        owner: None,
        drop_cleanup_timeout: timeout,
        drop_cleanup_probe: Some(probe),
    };

    let shutdown = shutdown_signal.notified();
    drop(runtime);
    command_seen.await.unwrap();
    tokio::time::advance(timeout).await;
    tokio::task::yield_now().await;

    assert_eq!(cleanup.await.unwrap(), DropCleanupOutcome::Escalated);
    shutdown.await;
    assert!(hub.lock().await.workflow_coordinator().is_none());
    assert_eq!(
        handle.shutdown().await,
        Err(WorkflowCoordinatorError::Unavailable)
    );
}

#[test]
fn runtime_errors_have_stable_display_messages() {
    let errors = [
        WorkflowRuntimeError::InvalidSettings("missing policy".into()),
        WorkflowRuntimeError::ProtectedAuditUnavailable,
        WorkflowRuntimeError::AlreadyAdmitted,
        WorkflowRuntimeError::AdmissionOccupied,
        WorkflowRuntimeError::Coordinator(WorkflowCoordinatorError::Unavailable),
        WorkflowRuntimeError::Tick(WorkflowCoordinatorError::JournalUnavailable),
        WorkflowRuntimeError::TaskJoin {
            task: "workflow actor",
            message: "cancelled".into(),
        },
    ];

    assert_eq!(
        errors.iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "invalid workflow settings: missing policy",
            "workflow protected audit is unavailable",
            "workflow runtime is already admitted",
            "another workflow runtime is already admitted",
            "workflow coordinator failed: workflow coordinator is unavailable",
            "workflow ticker failed: workflow journal is unavailable",
            "workflow actor task failed: cancelled",
        ]
    );
}
