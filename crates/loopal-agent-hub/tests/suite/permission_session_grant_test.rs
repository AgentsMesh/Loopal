use std::time::Duration;

use loopal_agent_hub::{UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::json;
use tokio::sync::mpsc;

#[tokio::test]
async fn session_permission_grant_skips_the_next_prompt_for_the_same_tool() {
    let (tx, rx) = mpsc::channel::<AgentEvent>(16);
    let hub = crate::permission_support::hub_with_noop_audit(tx);
    let _events = start_event_loop(hub.clone(), rx);
    let ui = UiSession::connect(
        hub.clone(),
        "ui-session-grant",
        loopal_protocol::UiCapabilities::ALL,
    )
    .await;
    let (agent, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let first_agent = agent.clone();
    let first = tokio::spawn(async move {
        first_agent
            .send_request(
                methods::AGENT_PERMISSION.name,
                crate::permission_request("write-1", "Write", json!({})),
            )
            .await
    });
    let mut event_rx = ui.event_rx;
    let event = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let event = event_rx.recv().await.unwrap();
            if matches!(
                event.payload,
                AgentEventPayload::ToolPermissionRequest { .. }
            ) {
                return event;
            }
        }
    })
    .await
    .unwrap();
    let (interaction_id, intent_digest) = match event.payload {
        AgentEventPayload::ToolPermissionRequest {
            id,
            permission_intent,
            ..
        } => (
            id,
            permission_intent
                .expect("Hub V2 event must include final permission intent")
                .intent_digest(),
        ),
        _ => unreachable!(),
    };
    ui.client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            json!({
                "agent_name": "agent-1", "tool_call_id": interaction_id,
                "permission_intent_digest": intent_digest,
                "allow": true, "remember_session": true,
            }),
        )
        .await
        .unwrap();
    assert_eq!(first.await.unwrap().unwrap()["allow"], true);

    let second = tokio::time::timeout(
        Duration::from_secs(1),
        agent.send_request(
            methods::AGENT_PERMISSION.name,
            crate::permission_request("write-2", "Write", json!({})),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(second["allow"], true);
    assert_eq!(second["allow"], true);
}
