//! Tests for permission/question relay from agent through Hub to UI clients.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::UiSession;
use loopal_agent_hub::hub_server;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(16);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// Single UI client: permission relayed and approved.
#[tokio::test]
async fn permission_relay_single_client() {
    let (hub, _event_rx) = make_hub();

    let ui = UiSession::connect(hub.clone(), "ui-1").await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");

    tokio::spawn(auto_respond_relay(
        ui.relay_rx,
        ui.client,
        json!({"allow": true}),
    ));
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = agent_conn
        .send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "t1", "tool_name": "Bash", "tool_input": {}}),
        )
        .await;

    assert!(result.is_ok(), "permission should succeed: {result:?}");
    assert_eq!(result.unwrap()["allow"], true);
}

/// Question relay to single UI client.
#[tokio::test]
async fn question_relay_single_client() {
    let (hub, _event_rx) = make_hub();

    let ui = UiSession::connect(hub.clone(), "ui-1").await;
    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");

    tokio::spawn(auto_respond_relay(
        ui.relay_rx,
        ui.client,
        json!({"answers": ["yes"]}),
    ));
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = agent_conn
        .send_request(methods::AGENT_QUESTION.name, json!({"questions": []}))
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap()["answers"][0], "yes");
}

/// Permission denied when no UI clients registered.
#[tokio::test]
async fn permission_denied_without_ui_client() {
    let (hub, _event_rx) = make_hub();

    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        agent_conn.send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "t1", "tool_name": "Bash", "tool_input": {}}),
        ),
    )
    .await;

    match result {
        Ok(Ok(resp)) => assert_eq!(resp["allow"], false),
        Ok(Err(e)) => panic!("request error: {e}"),
        Err(_) => panic!("TIMEOUT"),
    }
}

/// Race model: two UI clients registered, first response wins.
#[tokio::test]
async fn permission_race_first_response_wins() {
    let (hub, _event_rx) = make_hub();

    // Fast UI client: approves immediately
    let ui_fast = UiSession::connect(hub.clone(), "ui-fast").await;
    tokio::spawn(auto_respond_relay(
        ui_fast.relay_rx,
        ui_fast.client,
        json!({"allow": true}),
    ));

    // Slow UI client: responds after delay
    let ui_slow = UiSession::connect(hub.clone(), "ui-slow").await;
    tokio::spawn(delayed_respond_relay(
        ui_slow.relay_rx,
        ui_slow.client,
        json!({"allow": false}),
        Duration::from_millis(500),
    ));

    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = agent_conn
        .send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "t1", "tool_name": "Bash", "tool_input": {}}),
        )
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap()["allow"], true);
}

/// UI client lifecycle: register, check, unregister.
#[tokio::test]
async fn ui_client_lifecycle() {
    let (hub, _event_rx) = make_hub();

    assert!(!hub.lock().await.ui.is_ui_client("my-ui"));

    let _ui = UiSession::connect(hub.clone(), "my-ui").await;
    assert!(hub.lock().await.ui.is_ui_client("my-ui"));

    hub.lock().await.ui.unregister_client("my-ui");
    assert!(!hub.lock().await.ui.is_ui_client("my-ui"));
}

/// get_client_connections returns only registered UI clients.
#[tokio::test]
async fn get_client_connections_filters_correctly() {
    let (hub, _event_rx) = make_hub();

    let (_agent_conn, _) = hub_server::connect_local(hub.clone(), "agent-1");
    let _ui = UiSession::connect(hub.clone(), "my-ui").await;

    tokio::time::sleep(Duration::from_millis(30)).await;

    let ui_conns = hub.lock().await.ui.get_client_connections();
    assert_eq!(ui_conns.len(), 1);
    assert_eq!(ui_conns[0].0, "my-ui");
}

/// Auto-respond to relay requests via UiSession's relay_rx + HubClient.
async fn auto_respond_relay(
    mut rx: mpsc::Receiver<Incoming>,
    client: Arc<loopal_agent_hub::HubClient>,
    response: serde_json::Value,
) {
    while let Some(msg) = rx.recv().await {
        if let Incoming::Request { id, .. } = msg {
            let _ = client.connection().respond(id, response.clone()).await;
        }
    }
}

async fn delayed_respond_relay(
    mut rx: mpsc::Receiver<Incoming>,
    client: Arc<loopal_agent_hub::HubClient>,
    response: serde_json::Value,
    delay: Duration,
) {
    while let Some(msg) = rx.recv().await {
        if let Incoming::Request { id, .. } = msg {
            tokio::time::sleep(delay).await;
            let _ = client.connection().respond(id, response.clone()).await;
        }
    }
}
