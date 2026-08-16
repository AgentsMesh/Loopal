use std::collections::BTreeSet;
use std::sync::Arc;

use loopal_ipc::protocol::methods;
use loopal_ipc::{Connection, RpcError};
use loopal_protocol::{
    WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
};
use tokio::sync::{Mutex, mpsc};

use super::authorization;
use crate::Hub;
use crate::request_principal::{AgentPrincipal, HubRequestPrincipal};
use crate::types::{AgentOrigin, AgentRuntimeFacts, SpawnAuthority};

fn connection() -> Arc<Connection<loopal_ipc::Listening>> {
    let (_peer, transport) = loopal_ipc::duplex_pair();
    Connection::new(transport).into_listening().0
}

fn workflow_causation() -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_acl"),
        node_id: WorkflowNodeId::new("wnode_acl"),
        attempt_id: WorkflowAttemptId::new("watt_acl"),
    }
}

fn denied(error: RpcError) {
    assert!(error.to_string().contains("not authorized"), "{error}");
}

#[tokio::test]
async fn workflow_worker_has_a_separate_closed_acl_from_ordinary_children() {
    let (events, _rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution("worker", connection(), None, None, None)
        .unwrap();
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.origin = AgentOrigin::ManagedChild;
    facts.depth = 1;
    facts.workflow_permission_causation = Some(workflow_causation());
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&execution, facts.clone())
    );
    let workflow_worker = || {
        Arc::new(HubRequestPrincipal::Agent(AgentPrincipal::new(
            execution.clone(),
            facts.clone(),
        )))
    };

    let allowed = [
        methods::HUB_AUDIT_PROTECTED_EFFECT.name,
        methods::HUB_AUDIT_PERMISSION_DECISION.name,
        methods::HUB_WORKFLOW_PROVIDER_SECRET_GET.name,
        methods::HUB_WORKFLOW_WORKER_HANDSHAKE.name,
    ];
    for method in allowed {
        assert!(
            authorization::authorize(&hub, method, workflow_worker())
                .await
                .is_ok(),
            "workflow worker should be authorized for {method}"
        );
    }

    let dispatcher = crate::dispatch::build_hub_dispatcher(hub.clone());
    let allowed: BTreeSet<_> = allowed.into_iter().collect();
    for method in dispatcher
        .registered_methods()
        .into_iter()
        .filter(|method| !allowed.contains(method))
    {
        let error = match authorization::authorize(&hub, method, workflow_worker()).await {
            Ok(_) => panic!("workflow worker was authorized for {method}"),
            Err(error) => error,
        };
        denied(error);
    }
    for method in [methods::META_LIST_HUBS.name, methods::META_TOPOLOGY.name] {
        let error = match authorization::authorize(&hub, method, workflow_worker()).await {
            Ok(_) => panic!("workflow worker was authorized for {method}"),
            Err(error) => error,
        };
        denied(error);
    }

    let mut ordinary_facts = facts;
    ordinary_facts.workflow_permission_causation = None;
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&execution, ordinary_facts.clone())
    );
    let ordinary_child = Arc::new(HubRequestPrincipal::Agent(AgentPrincipal::new(
        execution,
        ordinary_facts,
    )));
    for method in [
        methods::HUB_ROUTE.name,
        methods::HUB_SPAWN_AGENT.name,
        methods::HUB_MCP_LIST_TOOLS.name,
        methods::HUB_SECRET_GET.name,
    ] {
        assert!(
            authorization::authorize(&hub, method, ordinary_child.clone())
                .await
                .is_ok(),
            "ordinary managed child compatibility changed for {method}"
        );
    }
}

#[tokio::test]
async fn raw_workflow_worker_connection_cannot_reach_privileged_handlers() {
    let (events, _rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let (worker_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (worker, _worker_rx) = Connection::new(worker_transport).into_listening();
    let (hub_connection, incoming) = Connection::new(hub_transport).into_listening();
    let execution = hub
        .lock()
        .await
        .registry
        .register_connection_with_parent_execution(
            "raw-workflow-worker",
            hub_connection.clone(),
            None,
            None,
            None,
        )
        .unwrap();
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.origin = AgentOrigin::ManagedChild;
    facts.depth = 1;
    facts.workflow_permission_causation = Some(workflow_causation());
    assert!(
        hub.lock()
            .await
            .registry
            .set_runtime_facts(&execution, facts)
    );
    let dispatcher = Arc::new(crate::dispatch::build_hub_dispatcher(hub.clone()));
    let io_task = tokio::spawn(crate::agent_io::agent_io_loop_exact(
        hub,
        dispatcher,
        hub_connection,
        incoming,
        "raw-workflow-worker".into(),
        execution,
    ));

    for method in [
        methods::HUB_ROUTE.name,
        methods::HUB_WAIT_AGENT.name,
        methods::HUB_LIST_AGENTS.name,
        methods::HUB_AGENT_INFO.name,
        methods::HUB_TOPOLOGY.name,
        methods::HUB_STATUS.name,
        methods::HUB_SPAWN_AGENT.name,
        methods::HUB_MCP_LIST_TOOLS.name,
        methods::HUB_MCP_CALL_TOOL.name,
        methods::HUB_MCP_SNAPSHOT.name,
        methods::HUB_SECRET_GET.name,
        methods::HUB_SECRET_LIST_NAMES.name,
        methods::HUB_SECRET_HEALTH.name,
        methods::META_LIST_HUBS.name,
    ] {
        denied(
            worker
                .send_request(method, serde_json::json!({"malicious": true}))
                .await
                .expect_err("raw workflow worker request must fail"),
        );
    }

    worker
        .send_notification(
            methods::AGENT_COMPLETED.name,
            serde_json::to_value(loopal_protocol::AgentCompletion::goal(None)).unwrap(),
        )
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), io_task)
        .await
        .unwrap()
        .unwrap();
}
