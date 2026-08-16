use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, UiCapabilities};
use serde_json::json;
use tokio::sync::mpsc;

#[tokio::test]
async fn interrupt_cancels_pending_without_clearing_session_grant() {
    let (tx, rx) = mpsc::channel::<AgentEvent>(32);
    let hub = crate::permission_support::hub_with_noop_audit(tx);
    let _event_loop = start_event_loop(hub.clone(), rx);
    let ui = UiSession::connect(
        hub.clone(),
        "desktop",
        UiCapabilities {
            permission: true,
            plan_approval: true,
            ..UiCapabilities::NONE
        },
    )
    .await;
    let (agent, mut agent_rx) = hub_server::connect_local(hub.clone(), "main");
    let interrupt_responder = agent.clone();
    tokio::spawn(async move {
        while let Some(message) = agent_rx.recv().await {
            if let loopal_ipc::connection::Incoming::Request { id, method, .. } = message
                && method == methods::AGENT_INTERRUPT.name
            {
                interrupt_responder
                    .respond(id, json!({"ok": true}))
                    .await
                    .unwrap();
            }
        }
    });

    let first_permission = request_permission(agent.clone(), "grant");
    let permission = crate::permission_interaction(&hub, "main", "grant").await;
    ui.client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            json!({
                "agent_name": "main", "tool_call_id": permission.id,
                "permission_intent_digest": permission.digest,
                "allow": true, "remember_session": true,
            }),
        )
        .await
        .unwrap();
    assert_eq!(first_permission.await.unwrap().unwrap()["allow"], true);

    let plan = tokio::spawn({
        let agent = agent.clone();
        async move {
            agent
                .send_request(
                    methods::AGENT_PLAN_APPROVAL.name,
                    json!({
                        "request_id": "interrupt", "plan_content": "# Plan",
                        "plan_path": "/tmp/plan.md",
                    }),
                )
                .await
        }
    });
    let _ = crate::plan_interaction_id(&hub, "main", "interrupt").await;
    ui.client.interrupt_target("main").await;
    let response = plan.await.unwrap().unwrap();
    assert_eq!(response["decision"], "cancelled");
    assert_eq!(response["reason"], "interrupted");

    let retained = tokio::time::timeout(
        Duration::from_secs(1),
        agent.send_request(
            methods::AGENT_PERMISSION.name,
            crate::permission_request("retained", "Bash", json!({})),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(retained["allow"], true);
}

fn request_permission(
    agent: Arc<loopal_ipc::Connection<loopal_ipc::Listening>>,
    id: &'static str,
) -> tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>> {
    tokio::spawn(async move {
        agent
            .send_request(
                methods::AGENT_PERMISSION.name,
                crate::permission_request(id, "Bash", json!({})),
            )
            .await
    })
}
