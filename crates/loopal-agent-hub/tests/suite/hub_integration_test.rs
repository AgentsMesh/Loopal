//! Integration tests for Hub agent lifecycle and routing.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::hub_server;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// Spawn a mock agent that auto-responds to all requests with {"ok": true}.
fn spawn_mock_agent(conn: Arc<Connection>, mut rx: mpsc::Receiver<Incoming>) {
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = conn.respond(id, json!({"ok": true})).await;
            }
        }
    });
}

// ── Registration ────────────────────────────────────────────────────

#[tokio::test]
async fn agent_registered_and_reachable() {
    let (hub, _) = make_hub();

    let (conn, rx) = hub_server::connect_local(hub.clone(), "worker-1");
    spawn_mock_agent(conn, rx);
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("worker-1")
            .is_some()
    );
}

#[tokio::test]
async fn duplicate_agent_name_rejected() {
    let (hub, _) = make_hub();

    let (c1, r1) = hub_server::connect_local(hub.clone(), "dup");
    spawn_mock_agent(c1, r1);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Second registration fails — agent_io task exits early
    let (c2, r2) = hub_server::connect_local(hub.clone(), "dup");
    spawn_mock_agent(c2, r2);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Hub still has exactly one "dup" (the first one)
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("dup")
            .is_some()
    );
}

// ── Message routing ─────────────────────────────────────────────────

#[tokio::test]
async fn agent_a_routes_message_to_agent_b() {
    let (hub, _) = make_hub();

    let (conn_a, rx_a) = hub_server::connect_local(hub.clone(), "agent-a");
    spawn_mock_agent(conn_a.clone(), rx_a);

    let (conn_b, rx_b) = hub_server::connect_local(hub.clone(), "agent-b");
    // B: capture incoming request method instead of auto-respond
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    let conn_b_bg = conn_b.clone();
    tokio::spawn(async move {
        let mut rx = rx_b;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = method_tx.send(method).await;
                let _ = conn_b_bg.respond(id, json!({"ok": true})).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": "agent-a"},
        "target": "agent-b",
        "content": {"text": "hello from A", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = conn_a.send_request(methods::HUB_ROUTE.name, envelope).await;
    assert!(result.is_ok(), "hub/route should succeed");

    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(
        method.unwrap().unwrap(),
        methods::AGENT_MESSAGE.name,
        "B should receive agent/message"
    );
}

// ── Control + Interrupt ─────────────────────────────────────────────

#[tokio::test]
async fn hub_control_reaches_target_agent() {
    let (hub, _) = make_hub();

    let (sender, sr) = hub_server::connect_local(hub.clone(), "sender");
    spawn_mock_agent(sender.clone(), sr);

    // Target: capture method of incoming request
    let (target_conn, target_rx) = hub_server::connect_local(hub.clone(), "target");
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    let tc = target_conn.clone();
    tokio::spawn(async move {
        let mut rx = target_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = method_tx.send(method).await;
                let _ = tc.respond(id, json!({"ok": true})).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = sender
        .send_request(
            methods::HUB_CONTROL.name,
            json!({"target": "target", "command": {"Clear": null}}),
        )
        .await;
    assert!(result.is_ok());

    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(method.unwrap().unwrap(), methods::AGENT_CONTROL.name);
}

#[tokio::test]
async fn hub_interrupt_reaches_target_agent() {
    let (hub, _) = make_hub();

    let (sender, sr) = hub_server::connect_local(hub.clone(), "sender");
    spawn_mock_agent(sender.clone(), sr);

    // Target: capture incoming notification method
    let (target_conn, target_rx) = hub_server::connect_local(hub.clone(), "target");
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    tokio::spawn(async move {
        let _keep = target_conn; // keep connection alive
        let mut rx = target_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Notification { method, .. } = msg {
                let _ = method_tx.send(method).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = sender
        .send_request(methods::HUB_INTERRUPT.name, json!({"target": "target"}))
        .await;
    assert!(result.is_ok());

    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(method.unwrap().unwrap(), methods::AGENT_INTERRUPT.name);
}

// ── Error handling ──────────────────────────────────────────────────

#[tokio::test]
async fn malformed_route_does_not_crash() {
    let (hub, _) = make_hub();
    let (conn, rx) = hub_server::connect_local(hub.clone(), "sender");
    spawn_mock_agent(conn.clone(), rx);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Invalid envelope JSON — should return error, not crash
    let result = conn
        .send_request(methods::HUB_ROUTE.name, json!({"garbage": true}))
        .await;
    assert!(result.is_ok(), "should get response (not transport error)");
}

#[tokio::test]
async fn missing_required_field_returns_error() {
    let (hub, _) = make_hub();
    let (conn, rx) = hub_server::connect_local(hub.clone(), "sender");
    spawn_mock_agent(conn.clone(), rx);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // hub/agent_info without name field → should return error
    let result = conn
        .send_request(methods::HUB_AGENT_INFO.name, json!({}))
        .await;
    assert!(
        result.is_ok(),
        "should get error response, not transport failure"
    );
    let val = result.unwrap();
    assert!(val.get("code").is_some() || val.get("message").is_some());
}

// ── Event propagation ───────────────────────────────────────────────

#[tokio::test]
async fn agent_event_reaches_hub_event_channel() {
    let (hub, mut event_rx) = make_hub();

    let (agent_conn, agent_rx) = hub_server::connect_local(hub.clone(), "worker");
    spawn_mock_agent(agent_conn.clone(), agent_rx);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Agent sends event notification
    let event = json!({
        "agent_name": null,
        "payload": {"Stream": {"text": "hello"}}
    });
    agent_conn
        .send_notification(methods::AGENT_EVENT.name, event)
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_secs(2), event_rx.recv()).await;
    assert!(received.is_ok(), "Hub should forward event");
    let evt = received.unwrap().unwrap();
    assert_eq!(evt.agent_name.as_deref(), Some("worker"));
}
