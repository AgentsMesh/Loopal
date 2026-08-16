use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use loopal_ipc::{Connection, protocol::methods};
use loopal_protocol::{
    PermissionIntentRequest, PermissionReceipt, ProtectedEffectAuditRequest,
    WorkflowAttemptCapability, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation,
    WorkflowRunId,
};
use loopal_tool_api::PermissionMode;
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

use crate::types::AgentOrigin;
use crate::{Hub, hub_server};

#[path = "permission_fast_path_authority_tests.rs"]
mod authority_tests;
#[path = "permission_fast_path_policy_tests.rs"]
mod policy_tests;

#[derive(Default)]
struct EffectCount(AtomicUsize);

impl loopal_vault_api::AuditSink for EffectCount {
    fn record(
        &self,
        _: loopal_vault_api::VaultOp,
        _: &str,
        _: &loopal_vault_api::AuditMetadata<'_>,
    ) -> loopal_vault_api::AuditResult<()> {
        Ok(())
    }

    fn record_protected(
        &self,
        op: loopal_vault_api::ProtectedOp,
        _: &str,
        _: &loopal_vault_api::AuditMetadata<'_>,
    ) -> loopal_vault_api::AuditResult<()> {
        if op == loopal_vault_api::ProtectedOp::ToolEffect {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

fn workflow() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_policy"),
        node_id: WorkflowNodeId::new("wnode_policy"),
        attempt_id: WorkflowAttemptId::new("watt_policy"),
    }
}

async fn wait_execution(hub: &Arc<Mutex<Hub>>, name: &str) -> crate::types::AgentExecutionRef {
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Some(execution) = hub.lock().await.registry.current_execution(name) {
                break execution;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap()
}

async fn connect_worker(
    hub: &Arc<Mutex<Hub>>,
    workflow: WorkflowPermissionCausation,
    mode: PermissionMode,
) -> (
    Arc<Connection<loopal_ipc::Listening>>,
    crate::types::AgentExecutionRef,
) {
    let parent = wait_execution(hub, "main").await;
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, _agent_rx) = Connection::new(agent_transport).into_listening();
    let (connection, incoming) = Connection::new(hub_transport).into_listening();
    let (execution, mut facts) = {
        let mut hub = hub.lock().await;
        let mut parent_facts = hub.registry.runtime_facts(&parent).unwrap().clone();
        parent_facts.session_id = Some("session-policy".into());
        assert!(
            hub.registry
                .set_runtime_facts(&parent, parent_facts.clone())
        );
        let execution = hub
            .registry
            .register_connection_with_exact_parent_execution(
                "worker",
                connection.clone(),
                Some(loopal_protocol::QualifiedAddress::local("main")),
                Some(&parent),
                None,
                None,
                false,
            )
            .unwrap();
        (execution, parent_facts)
    };
    facts.origin = AgentOrigin::ManagedChild;
    facts.root = "main".into();
    facts.parent = Some(parent);
    facts.depth = 1;
    facts.workflow_permission_causation = Some(workflow);
    facts.workflow_attempt_capability_digest = Some(WorkflowAttemptCapability::generate().digest());
    facts.spawn.permission_mode = mode;
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&execution, facts)
    );
    crate::agent_io::spawn_io_loop_exact(
        hub.clone(),
        Arc::new(crate::dispatch::build_hub_dispatcher(hub.clone())),
        "worker",
        connection,
        incoming,
        execution.clone(),
    );
    (agent, execution)
}

fn request(workflow: WorkflowPermissionCausation) -> PermissionIntentRequest {
    PermissionIntentRequest::create(
        "effect",
        "Bash",
        json!({"command": "true"}),
        json!({"command": "true"}),
        json!({"type": "object"}),
        Some(workflow),
    )
    .unwrap()
}

#[tokio::test]
async fn bypass_workflow_gets_receipt_without_ui_and_consumes_once() {
    let (events, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let audit = Arc::new(EffectCount::default());
    hub.lock().await.set_protected_audit(audit.clone());
    let (_root, _root_rx) = hub_server::connect_local(hub.clone(), "main");
    let workflow = workflow();
    let (agent, execution) = connect_worker(&hub, workflow.clone(), PermissionMode::Bypass).await;
    let permission = request(workflow.clone());
    let response = agent
        .send_request(
            methods::AGENT_PERMISSION.name,
            serde_json::to_value(&permission).unwrap(),
        )
        .await
        .unwrap();
    let receipt: PermissionReceipt =
        serde_json::from_value(response["permission_receipt"].clone()).unwrap();
    assert_eq!(response["allow"], true);
    assert_eq!(receipt.workflow(), Some(&workflow));
    assert_eq!(
        receipt.execution_generation(),
        execution.connection_generation
    );
    assert!(receipt.interaction_token().starts_with("policy:"));

    let effect = ProtectedEffectAuditRequest::new(
        "effect",
        "Bash",
        permission.intent_seed.action_digest(),
        permission.intent_seed.schema_digest(),
    )
    .unwrap()
    .with_receipt(receipt);
    let params = serde_json::to_value(effect).unwrap();
    let recorded = agent
        .send_request(methods::HUB_AUDIT_PROTECTED_EFFECT.name, params.clone())
        .await
        .unwrap();
    assert_eq!(recorded, json!({"recorded": true}));
    let replay = agent
        .send_request(methods::HUB_AUDIT_PROTECTED_EFFECT.name, params)
        .await
        .unwrap_err();
    assert!(replay.to_string().contains("already consumed"));
    assert_eq!(audit.0.load(Ordering::SeqCst), 1);
}
