use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_protocol::{
    PermissionIntentRequest, UiCapabilities, WorkflowAttemptId, WorkflowNodeId,
    WorkflowPermissionCausation, WorkflowRunId,
};
use loopal_tool_api::PermissionMode;
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

use crate::spawn_manager::spawn_audit_test_support::Sink;
use crate::types::{AgentExecutionRef, AgentRuntimeFacts, SpawnAuthority};
use crate::{Hub, UiSession, hub_server, start_event_loop};

async fn set_workflow_authority(
    hub: &Arc<Mutex<Hub>>,
    workflow: WorkflowPermissionCausation,
    mode: PermissionMode,
) -> crate::types::AgentExecutionRef {
    let mut hub = hub.lock().await;
    let execution = hub.registry.current_execution("main").unwrap();
    let mut facts = hub.registry.runtime_facts(&execution).unwrap().clone();
    facts.workflow_permission_causation = Some(workflow);
    facts.spawn.permission_mode = mode;
    assert!(hub.registry.set_runtime_facts(&execution, facts));
    execution
}

fn request(id: &str, workflow: Option<WorkflowPermissionCausation>) -> serde_json::Value {
    serde_json::to_value(
        PermissionIntentRequest::create(
            id,
            "Write",
            json!({}),
            json!({}),
            json!({"type": "object", "required": ["file_path"]}),
            workflow,
        )
        .unwrap(),
    )
    .unwrap()
}

fn workflow() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_test"),
        node_id: WorkflowNodeId::new("wnode_test"),
        attempt_id: WorkflowAttemptId::new("watt_test"),
    }
}

async fn pending(
    hub: &Arc<Mutex<Hub>>,
    logical_id: &str,
) -> (String, loopal_protocol::PermissionIntentDigest, u64, u64) {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Some(info) = hub
                .lock()
                .await
                .pending_permissions
                .get(&("main".into(), logical_id.into()))
            {
                return (
                    info.interaction_id.clone(),
                    info.permission_intent.intent_digest(),
                    info.permission_intent.execution_generation(),
                    info.permission_intent.ui_generation(),
                );
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

include!("handle_tests/workflow_prompt.rs");

async fn exact_connection_fixture(
    audit: Arc<dyn loopal_vault_api::AuditSink>,
) -> (
    Arc<Mutex<Hub>>,
    Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    AgentExecutionRef,
) {
    let (events, _event_rx) = mpsc::channel(8);
    let mut hub = Hub::new(events);
    hub.set_protected_audit(audit);
    let (peer_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (peer, _peer_rx) = loopal_ipc::Connection::new(peer_transport).into_listening();
    let (connection, _hub_rx) = loopal_ipc::Connection::new(hub_transport).into_listening();
    let execution = hub
        .registry
        .register_connection_with_parent_execution("main", connection.clone(), None, None, None)
        .unwrap();
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.session_id = Some("session-permission".into());
    assert!(hub.registry.set_runtime_facts(&execution, facts));
    (Arc::new(Mutex::new(hub)), peer, connection, execution)
}

include!("handle_tests/authority.rs");
