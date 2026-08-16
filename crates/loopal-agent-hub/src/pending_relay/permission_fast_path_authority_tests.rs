use loopal_ipc::protocol::methods;
use loopal_protocol::WorkflowAttemptCapability;
use loopal_tool_api::PermissionMode;
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

use super::{request, wait_execution, workflow};
use crate::{Hub, hub_server};

#[tokio::test]
async fn root_cannot_get_policy_receipt_by_forging_workflow_facts() {
    let (events, _event_rx) = mpsc::channel(8);
    let hub = std::sync::Arc::new(Mutex::new(Hub::new(events)));
    hub.lock()
        .await
        .set_protected_audit(std::sync::Arc::new(loopal_vault_api::NoopAuditSink));
    let (agent, _agent_rx) = hub_server::connect_local(hub.clone(), "main");
    let workflow = workflow();
    let execution = wait_execution(&hub, "main").await;
    let mut facts = hub
        .lock()
        .await
        .registry
        .runtime_facts(&execution)
        .unwrap()
        .clone();
    facts.workflow_permission_causation = Some(workflow.clone());
    facts.workflow_attempt_capability_digest = Some(WorkflowAttemptCapability::generate().digest());
    facts.spawn.permission_mode = PermissionMode::Bypass;
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&execution, facts)
    );

    let response = agent
        .send_request(
            methods::AGENT_PERMISSION.name,
            serde_json::to_value(request(workflow)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response, json!({"allow": false}));
    assert_eq!(hub.lock().await.permission_receipts.len(), 0);
}
