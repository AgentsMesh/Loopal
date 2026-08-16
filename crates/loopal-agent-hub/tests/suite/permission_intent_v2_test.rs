use std::sync::Arc;

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{
    AgentEvent, PermissionIntentDigest, UiCapabilities, WorkflowAttemptId, WorkflowNodeId,
    WorkflowPermissionCausation, WorkflowRunId,
};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (events, receiver) = mpsc::channel(32);
    (
        crate::permission_support::hub_with_noop_audit(events),
        receiver,
    )
}

async fn setup() -> (
    Arc<Mutex<Hub>>,
    UiSession,
    Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
) {
    let (hub, events) = make_hub();
    let _event_loop = start_event_loop(hub.clone(), events);
    let ui = UiSession::connect(hub.clone(), "desktop", UiCapabilities::ALL).await;
    let (agent, _) = hub_server::connect_local(hub.clone(), "main");
    (hub, ui, agent)
}

fn send_permission(
    agent: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    request: serde_json::Value,
) -> tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>> {
    tokio::spawn(async move {
        agent
            .send_request(methods::AGENT_PERMISSION.name, request)
            .await
    })
}

#[tokio::test]
async fn legacy_and_partial_v2_requests_cannot_downgrade() {
    let (hub, _ui, agent) = setup().await;
    let legacy = agent
        .send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "legacy", "tool_name": "Bash", "tool_input": {}}),
        )
        .await
        .unwrap();
    assert_eq!(legacy["allow"], false);

    let mut partial = crate::permission_request("partial", "Bash", json!({}));
    partial.as_object_mut().unwrap().remove("action_input");
    let partial = agent
        .send_request(methods::AGENT_PERMISSION.name, partial)
        .await
        .unwrap();
    assert_eq!(partial["allow"], false);
    assert!(hub.lock().await.pending_permissions.is_empty());
}

#[tokio::test]
async fn action_mutation_and_wrong_digest_fail_closed() {
    let (hub, ui, agent) = setup().await;
    let mut tampered =
        crate::permission_request("tampered", "Bash", json!({"command": "printf safe"}));
    tampered["action_input"]["command"] = json!("rm -rf /tmp/target");
    let denied = agent
        .send_request(methods::AGENT_PERMISSION.name, tampered)
        .await
        .unwrap();
    assert_eq!(denied["allow"], false);

    let request = send_permission(
        agent,
        crate::permission_request("wrong-digest", "Bash", json!({})),
    );
    let interaction = crate::permission_interaction(&hub, "main", "wrong-digest").await;
    ui.client
        .respond_permission(
            "main",
            &interaction.id,
            Some(PermissionIntentDigest::from_bytes([0x5a; 32])),
            true,
        )
        .await;
    assert_eq!(request.await.unwrap().unwrap()["allow"], false);
    assert!(hub.lock().await.pending_permissions.is_empty());
}

#[tokio::test]
async fn ui_topology_change_invalidates_allow() {
    let (hub, ui, agent) = setup().await;
    let request = send_permission(
        agent,
        crate::permission_request("topology", "Write", json!({})),
    );
    let interaction = crate::permission_interaction(&hub, "main", "topology").await;
    let _observer = UiSession::connect(hub.clone(), "observer", UiCapabilities::NONE).await;
    ui.client
        .respond_permission("main", &interaction.id, Some(interaction.digest), true)
        .await;
    assert_eq!(request.await.unwrap().unwrap()["allow"], false);
}

#[tokio::test]
async fn grant_is_schema_scoped_and_forged_workflow_is_rejected() {
    let (hub, ui, agent) = setup().await;
    let first = send_permission(
        agent.clone(),
        crate::permission_support::permission_request_with(
            "grant",
            "Write",
            json!({}),
            json!({"type": "object", "required": ["file_path"]}),
            None,
        ),
    );
    let grant = crate::permission_interaction(&hub, "main", "grant").await;
    ui.client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            json!({
                "agent_name": "main",
                "tool_call_id": grant.id,
                "permission_intent_digest": grant.digest,
                "allow": true,
                "remember_session": true,
            }),
        )
        .await
        .unwrap();
    assert_eq!(first.await.unwrap().unwrap()["allow"], true);

    let mut malformed = crate::permission_support::permission_request_with(
        "malformed-granted",
        "Write",
        json!({}),
        json!({"type": "object", "required": ["file_path"]}),
        None,
    );
    malformed.as_object_mut().unwrap().remove("action_input");
    let malformed = agent
        .send_request(methods::AGENT_PERMISSION.name, malformed)
        .await
        .unwrap();
    assert_eq!(malformed["allow"], false);
    assert!(hub.lock().await.pending_permissions.is_empty());

    let schema_changed = send_permission(
        agent.clone(),
        crate::permission_support::permission_request_with(
            "schema-changed",
            "Write",
            json!({}),
            json!({"type": "object", "required": ["content"]}),
            None,
        ),
    );
    let changed = crate::permission_interaction(&hub, "main", "schema-changed").await;
    ui.client
        .respond_permission("main", &changed.id, Some(changed.digest), false)
        .await;
    assert_eq!(schema_changed.await.unwrap().unwrap()["allow"], false);

    let workflow = WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_1"),
        node_id: WorkflowNodeId::new("wnode_1"),
        attempt_id: WorkflowAttemptId::new("watt_1"),
    };
    let workflow_request = send_permission(
        agent,
        crate::permission_support::permission_request_with(
            "workflow",
            "Write",
            json!({}),
            json!({"type": "object", "required": ["file_path"]}),
            Some(workflow),
        ),
    );
    assert_eq!(workflow_request.await.unwrap().unwrap()["allow"], false);
    assert!(hub.lock().await.pending_permissions.is_empty());
}
