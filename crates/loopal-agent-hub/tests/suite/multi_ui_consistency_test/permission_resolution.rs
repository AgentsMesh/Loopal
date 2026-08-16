use std::sync::Arc;

use loopal_agent_hub::hub_server;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_ipc::{Connection, Listening};
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::json;
use tokio::sync::mpsc;

use super::{connect_ui, make_hub, next_event_matching};

fn approve_first_permission_via_events(
    conn: Arc<Connection<Listening>>,
    mut rx: mpsc::Receiver<Incoming>,
    allow: bool,
) {
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let Incoming::Notification { method, params } = msg else {
                continue;
            };
            if method != methods::AGENT_EVENT.name {
                continue;
            }
            let Ok(event) = serde_json::from_value::<AgentEvent>(params) else {
                continue;
            };
            let agent = event
                .agent_name
                .as_ref()
                .map(|qualified| qualified.agent.clone())
                .unwrap_or_else(|| "main".to_string());
            if let AgentEventPayload::ToolPermissionRequest {
                id,
                permission_intent,
                ..
            } = event.payload
            {
                let digest = permission_intent.map(|intent| intent.intent_digest());
                let _ = conn
                    .send_request(
                        methods::HUB_PERMISSION_RESPONSE.name,
                        json!({
                            "agent_name": agent,
                            "tool_call_id": id,
                            "permission_intent_digest": digest,
                            "allow": allow,
                        }),
                    )
                    .await;
                return;
            }
        }
    });
}

#[tokio::test]
async fn permission_resolved_event_reaches_non_winning_ui() {
    let (hub, port, token) = make_hub().await;
    let (agent_side, _agent_rx) = hub_server::connect_local(hub.clone(), "main");

    let (ui_a, rx_a) = connect_ui(port, &token, "tui-A").await;
    let (_ui_b, mut rx_b) = connect_ui(port, &token, "tui-B").await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    approve_first_permission_via_events(ui_a, rx_a, true);

    let permission = crate::permission_request("perm-1", "Bash", json!({"command": "ls"}));
    let response = tokio::spawn(async move {
        agent_side
            .send_request(methods::AGENT_PERMISSION.name, permission)
            .await
    });
    let requested = next_event_matching(&mut rx_b, |payload| {
        matches!(payload, AgentEventPayload::ToolPermissionRequest { .. })
    })
    .await;
    let AgentEventPayload::ToolPermissionRequest {
        id: interaction_id, ..
    } = requested.payload
    else {
        unreachable!()
    };
    let response = response
        .await
        .expect("agent permission task panicked")
        .expect("agent gets permission response");
    assert_eq!(
        response.get("allow").and_then(|value| value.as_bool()),
        Some(true)
    );

    let resolved = next_event_matching(&mut rx_b, |payload| {
        matches!(payload, AgentEventPayload::ToolPermissionResolved { .. })
    })
    .await;
    let AgentEventPayload::ToolPermissionResolved { id } = resolved.payload else {
        panic!("expected resolved permission")
    };
    assert_eq!(id, interaction_id);
}
