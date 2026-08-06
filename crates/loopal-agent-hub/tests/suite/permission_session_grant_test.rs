use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::{Hub, UiSession, hub_server, start_event_loop};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

#[tokio::test]
async fn session_permission_grant_skips_the_next_prompt_for_the_same_tool() {
    let (tx, rx) = mpsc::channel::<AgentEvent>(16);
    let hub = Arc::new(Mutex::new(Hub::new(tx)));
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
                json!({
                    "tool_call_id": "write-1", "tool_name": "Write", "tool_input": {},
                }),
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
    let interaction_id = match event.payload {
        AgentEventPayload::ToolPermissionRequest { id, .. } => id,
        _ => unreachable!(),
    };
    ui.client
        .connection()
        .send_request(
            methods::HUB_PERMISSION_RESPONSE.name,
            json!({
                "agent_name": "agent-1", "tool_call_id": interaction_id,
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
            json!({"tool_call_id": "write-2", "tool_name": "Write", "tool_input": {}}),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(second["allow"], true);
    assert!(
        hub.lock()
            .await
            .session_permission_grants
            .contains(&("agent-1".into(), "Write".into()))
    );
}
