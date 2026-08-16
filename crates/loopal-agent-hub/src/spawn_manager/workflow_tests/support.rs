use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use loopal_ipc::Connection;
use loopal_protocol::{AgentEvent, QualifiedAddress, WorkflowPermissionCausation};
use loopal_vault_api::AuditSink;
use tokio::sync::{Mutex, mpsc};

use super::super::{AttemptOwner, AttemptPhase, ProductionWorkflowSpawner};
use super::fake_worker::{FakeProcess, Probe, spawn_peer};
use super::requests::causation;
use crate::Hub;
use crate::spawn_manager::spawn::{WorkflowProcessOwner, prepare_and_register_process};
use crate::spawn_manager::{PreparedSpawn, SpawnRequestLease};
use crate::types::{AgentExecutionRef, SpawnAuthority};
use crate::workflow::WorkflowOwner;

pub(super) struct Harness {
    pub(super) spawner: Arc<ProductionWorkflowSpawner>,
    pub(super) execution: AgentExecutionRef,
    pub(super) causation: WorkflowPermissionCausation,
    pub(super) probe: Arc<Probe>,
}

pub(super) async fn harness() -> Harness {
    harness_with_audit(Some(Arc::new(loopal_vault_api::NoopAuditSink))).await
}

pub(super) async fn harness_with_audit(audit: Option<Arc<dyn AuditSink>>) -> Harness {
    let (events, mut event_rx) = mpsc::channel::<AgentEvent>(16);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let mut hub = Hub::new(events);
    if let Some(audit) = audit {
        hub.set_protected_audit(audit);
    }
    let shutdown_signal = hub.shutdown_signal.clone();
    let hub = Arc::new(Mutex::new(hub));
    let spawner = ProductionWorkflowSpawner::new(hub.clone(), shutdown_signal);
    let probe = Arc::new(Probe::default());
    probe.reply_to_shutdown.store(true, Ordering::SeqCst);
    let (child_transport, hub_transport) = loopal_ipc::duplex_pair();
    spawn_peer(child_transport, probe.clone());
    let process = FakeProcess::new(hub_transport, probe.clone());
    let prepared = prepare_and_register_process(hub, spawn(), process)
        .await
        .unwrap();
    let execution = prepared.registered.execution.clone();
    let (process, control) = prepared.into_workflow_parts();
    let causation = causation("wrun_owner", "wnode_owner", "watt_owner");
    install_owner(
        &spawner,
        execution.clone(),
        causation.clone(),
        process,
        control,
    )
    .await;
    Harness {
        spawner,
        execution,
        causation,
        probe,
    }
}

pub(super) async fn spawner_with_root(
    audit: Option<Arc<dyn AuditSink>>,
) -> Arc<ProductionWorkflowSpawner> {
    let (events, mut event_rx) = mpsc::channel::<AgentEvent>(16);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let mut hub = Hub::new(events);
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let execution = hub
        .registry
        .register_connection_with_parent_execution("root", connection, None, None, None)
        .unwrap();
    let mut facts =
        crate::types::AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.session_id = Some("session".into());
    assert!(hub.registry.set_runtime_facts(&execution, facts));
    if let Some(audit) = audit {
        hub.set_protected_audit(audit);
    }
    let shutdown_signal = hub.shutdown_signal.clone();
    ProductionWorkflowSpawner::new(Arc::new(Mutex::new(hub)), shutdown_signal)
}

async fn install_owner(
    spawner: &ProductionWorkflowSpawner,
    execution: AgentExecutionRef,
    causation: WorkflowPermissionCausation,
    process: WorkflowProcessOwner,
    control: crate::spawn_manager::spawn::PreparedControl,
) {
    let mut owners = spawner.attempts.lock().await;
    owners
        .by_execution
        .insert(execution.clone(), causation.attempt_id.clone());
    owners.by_attempt.insert(
        causation.attempt_id.clone(),
        AttemptOwner {
            owner: WorkflowOwner::new("session", QualifiedAddress::local("root")),
            causation,
            execution,
            control: Arc::new(control),
            process: Some(process),
            process_shutdown: None,
            cleanup_registered: false,
            operation: Arc::new(Mutex::new(())),
            phase: AttemptPhase::Prepared,
        },
    );
}

fn spawn() -> PreparedSpawn {
    let cwd = std::env::temp_dir();
    PreparedSpawn {
        name: "workflow-watt_owner".into(),
        request_lease: SpawnRequestLease::Internal,
        cwd: cwd.clone(),
        prompt: Some("task".into()),
        parent: None,
        parent_execution: None,
        authority: SpawnAuthority::default(),
        agent_type: Some("default".into()),
        depth: 1,
        fork_context: None,
        workflow_permission_causation: None,
        workflow_attempt_capability: None,
        workflow_completion_result_limit: None,
        notify_parent_on_completion: false,
        root_cwd: cwd,
        root: "root".into(),
    }
}

pub(super) async fn wait_for(value: &AtomicUsize, expected: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while value.load(Ordering::SeqCst) != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
