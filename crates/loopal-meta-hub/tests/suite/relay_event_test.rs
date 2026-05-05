//! Local-UI permission flow with uplink present.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use serde_json::json;

use crate::test_helpers::*;

/// In the event-driven model, Sub-Hub no longer relays permission to MetaHub.
/// With both an uplink and a local UI registered, the local UI's response is
/// what reaches the agent — the uplink is unrelated to permission flow.
#[tokio::test]
async fn local_ui_responds_with_uplink_present() {
    let (hub, raw_rx) = make_hub();
    let _event_loop = loopal_agent_hub::start_event_loop(hub.clone(), raw_rx);

    {
        let (t, _) = loopal_ipc::duplex_pair();
        let c = Arc::new(Connection::new(t));
        let _rx = c.start();
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(c, "hub-x".into()));
        hub.lock().await.uplink = Some(ul);
    }

    let ui = loopal_agent_hub::UiSession::connect(hub.clone(), "local-ui").await;
    let ui_client = ui.client.clone();
    tokio::spawn(async move {
        let mut event_rx = ui.event_rx;
        while let Ok(ev) = event_rx.recv().await {
            if let loopal_protocol::AgentEventPayload::ToolPermissionRequest { id, .. } = ev.payload
            {
                let agent = ev
                    .agent_name
                    .as_ref()
                    .map(|q| q.agent.clone())
                    .unwrap_or_else(|| "main".to_string());
                ui_client.respond_permission(&agent, &id, true).await;
                return;
            }
        }
    });

    let (ac, agent_rx) = loopal_agent_hub::hub_server::connect_local(hub.clone(), "agent");
    tokio::spawn(async move {
        let mut rx = agent_rx;
        while rx.recv().await.is_some() {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let resp = ac
        .send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "t-local", "tool_name": "Bash", "tool_input": {}}),
        )
        .await
        .unwrap();
    assert_eq!(resp["allow"].as_bool(), Some(true));
}
