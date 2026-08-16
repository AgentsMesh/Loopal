use std::time::Duration;

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation,
    WorkflowRunId,
};
use tokio::sync::{Mutex, mpsc};

use super::*;

pub(super) async fn hub_with_uplink(
    event_tx: mpsc::Sender<AgentEvent>,
) -> (
    Arc<Mutex<Hub>>,
    Arc<Connection<loopal_ipc::Listening>>,
    mpsc::Receiver<Incoming>,
    AgentExecutionRef,
) {
    hub_with_uplink_and_audit(event_tx, Some(Arc::new(loopal_vault_api::NoopAuditSink))).await
}

pub(super) async fn hub_with_uplink_and_audit(
    event_tx: mpsc::Sender<AgentEvent>,
    audit: Option<Arc<dyn loopal_vault_api::AuditSink>>,
) -> (
    Arc<Mutex<Hub>>,
    Arc<Connection<loopal_ipc::Listening>>,
    mpsc::Receiver<Incoming>,
    AgentExecutionRef,
) {
    let (hub_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (hub_connection, _hub_rx) = Connection::new(hub_transport).into_listening();
    let (meta_connection, meta_rx) = Connection::new(meta_transport).into_listening();
    let (_requester_peer, requester_transport) = loopal_ipc::duplex_pair();
    let (requester_connection, _requester_rx) =
        Connection::new(requester_transport).into_listening();
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let requester = {
        let mut locked = hub.lock().await;
        locked.uplink = Some(Arc::new(HubUplink::new(hub_connection, "origin".into())));
        let execution = locked
            .registry
            .register_connection_with_parent_execution(
                "main",
                requester_connection,
                None,
                None,
                None,
            )
            .unwrap();
        let mut facts = crate::types::AgentRuntimeFacts::root(
            locked.default_cwd.clone(),
            crate::types::SpawnAuthority::default(),
        );
        facts.session_id = Some("session-cross-hub".into());
        facts.workflow_permission_causation = Some(WorkflowPermissionCausation {
            run_id: WorkflowRunId::new("wrun_cross_hub"),
            node_id: WorkflowNodeId::new("wnode_cross_hub"),
            attempt_id: WorkflowAttemptId::new("watt_cross_hub"),
        });
        assert!(locked.registry.set_runtime_facts(&execution, facts));
        if let Some(audit) = audit {
            locked.set_protected_audit(audit);
        }
        execution
    };
    (hub, meta_connection, meta_rx, requester)
}

pub(super) fn signed_spawn(name: &str) -> Value {
    json!({
        "name": name,
        "model": "test-model",
        "parent": "origin/main",
        "depth": 1,
        "permission_mode": "ask_any_write",
        "decision_mode": "manual",
        "sandbox_policy": "read_only",
        "no_sandbox": false,
        "target_hub": "destination",
    })
}

fn respond_to_spawn(
    meta_connection: Arc<Connection<loopal_ipc::Listening>>,
    mut meta_rx: mpsc::Receiver<Incoming>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let Incoming::Request { id, method, .. } = meta_rx.recv().await.unwrap() else {
            panic!("expected meta/spawn request");
        };
        assert_eq!(method, methods::META_SPAWN.name);
        meta_connection
            .respond(id, json!({"agent_id": "remote-id"}))
            .await
            .unwrap();
    })
}

include!("cross_hub_forward_tests/admission.rs");
include!("cross_hub_forward_tests/unknown_outcome.rs");
include!("cross_hub_forward_tests/lease_races.rs");
include!("cross_hub_forward_tests/completion.rs");
