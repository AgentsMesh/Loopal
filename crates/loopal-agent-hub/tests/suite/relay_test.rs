//! Tests for permission/question relay from agent through Hub to TUI.

use std::sync::Arc;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::AgentHub;
use loopal_agent_hub::hub_server;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<AgentHub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(16);
    (Arc::new(Mutex::new(AgentHub::new(tx))), rx)
}

/// Permission request relayed from agent → Hub → TUI → Hub → agent.
#[tokio::test]
async fn permission_relay_to_tui() {
    let (hub, _event_rx) = make_hub();

    // Create TUI + agent local connections
    let (tui_conn, tui_rx) = hub_server::connect_local(hub.clone(), "_tui");
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");

    // TUI auto-approves permission requests
    tokio::spawn(auto_respond(tui_conn, tui_rx, json!({"allow": true})));

    // Small delay to let connections register
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let result = agent_conn
        .send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "t1", "tool_name": "Bash", "tool_input": {}}),
        )
        .await;

    assert!(result.is_ok(), "permission should succeed: {result:?}");
    assert_eq!(result.unwrap()["allow"], true);
}

/// Question request relayed from agent → Hub → TUI.
#[tokio::test]
async fn question_relay_to_tui() {
    let (hub, _event_rx) = make_hub();

    let (tui_conn, tui_rx) = hub_server::connect_local(hub.clone(), "_tui");
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");

    tokio::spawn(auto_respond(tui_conn, tui_rx, json!({"answers": ["yes"]})));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let result = agent_conn
        .send_request(methods::AGENT_QUESTION.name, json!({"questions": []}))
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap()["answers"][0], "yes");
}

/// Permission denied when no TUI connected.
#[tokio::test]
async fn permission_denied_without_tui() {
    let (hub, _event_rx) = make_hub();

    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");

    // Wait for agent_io_loop to start processing
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        agent_conn.send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "t1", "tool_name": "Bash", "tool_input": {}}),
        ),
    )
    .await;

    match result {
        Ok(Ok(resp)) => assert_eq!(resp["allow"], false),
        Ok(Err(e)) => panic!("request error: {e}"),
        Err(_) => panic!("TIMEOUT: agent_io_loop did not process permission request"),
    }
}

async fn auto_respond(
    conn: Arc<Connection>,
    mut rx: mpsc::Receiver<Incoming>,
    response: serde_json::Value,
) {
    while let Some(msg) = rx.recv().await {
        if let Incoming::Request { id, .. } = msg {
            let _ = conn.respond(id, response.clone()).await;
        }
    }
}
