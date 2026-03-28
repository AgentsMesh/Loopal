//! Integration tests for spawn lifecycle using mock transport.
//!
//! Uses `register_agent_connection` with duplex pairs instead of real processes,
//! enabling full spawn/wait/route testing without forking.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::AgentHub;
use loopal_agent_hub::hub_server;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use serde_json::json;

fn make_hub() -> (Arc<Mutex<AgentHub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(AgentHub::new(tx))), rx)
}

// ── Spawn + register via mock transport ─────────────────────────────

#[tokio::test]
async fn register_agent_connection_makes_agent_routable() {
    let (hub, mut event_rx) = make_hub();

    // Create mock agent connection (duplex pair)
    let (agent_client, agent_server) = loopal_ipc::duplex_pair();
    let agent_conn = Arc::new(Connection::new(agent_client));
    let server_conn = Arc::new(Connection::new(agent_server));

    let agent_rx = agent_conn.start();
    let server_rx = server_conn.start();

    // Register via testable API
    let agent_id = register_agent_connection(
        hub.clone(),
        "mock-worker",
        server_conn,
        server_rx,
        None,
        None,
    )
    .await;
    assert!(!agent_id.is_empty());

    // Should receive SubAgentSpawned event
    let event = tokio::time::timeout(Duration::from_secs(1), event_rx.recv()).await;
    assert!(event.is_ok());
    let evt = event.unwrap().unwrap();
    if let AgentEventPayload::SubAgentSpawned { name, .. } = evt.payload {
        assert_eq!(name, "mock-worker");
    } else {
        panic!("expected SubAgentSpawned, got {:?}", evt.payload);
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Agent should be registered and routable
    assert!(
        hub.lock()
            .await
            .get_agent_connection("mock-worker")
            .is_some()
    );

    // Mock agent responds to requests
    let ac = agent_conn.clone();
    tokio::spawn(async move {
        let mut rx = agent_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = ac.respond(id, json!({"received": true})).await;
            }
        }
    });

    // Another agent can route to mock-worker
    let (sender_conn, sr) = hub_server::connect_local(hub.clone(), "sender");
    let sc = sender_conn.clone();
    tokio::spawn(async move {
        let mut rx = sr;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = sc.respond(id, json!({"ok": true})).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": "sender"},
        "target": "mock-worker",
        "content": {"text": "hello mock", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = sender_conn
        .send_request(methods::HUB_ROUTE.name, envelope)
        .await;
    assert!(result.is_ok(), "should route to mock agent");
}

// ── Wait for agent completion ───────────────────────────────────────

#[tokio::test]
async fn wait_agent_returns_when_agent_disconnects() {
    let (hub, _event_rx) = make_hub();

    // Create mock agent
    let (agent_client, agent_server) = loopal_ipc::duplex_pair();
    let agent_conn = Arc::new(Connection::new(agent_client));
    let server_conn = Arc::new(Connection::new(agent_server));

    let _agent_rx = agent_conn.start();
    let server_rx = server_conn.start();

    register_agent_connection(hub.clone(), "ephemeral", server_conn, server_rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Start waiting in background
    let hub_wait = hub.clone();
    let wait_handle = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &hub_wait,
            methods::HUB_WAIT_AGENT.name,
            json!({"name": "ephemeral"}),
            "waiter".into(),
        )
        .await
    });

    // Give wait_agent time to set up watcher
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Simulate agent completion (in production, this happens when stdio closes)
    {
        let mut h = hub.lock().await;
        h.emit_agent_finished("ephemeral", Some("test output".into()));
    }

    // Wait should complete
    let result = tokio::time::timeout(Duration::from_secs(3), wait_handle).await;
    assert!(result.is_ok(), "wait should complete after disconnect");
    let inner = result.unwrap().unwrap();
    assert!(inner.is_ok(), "should return Ok");
}

// ── Spawned agent sends hub/route back to parent ────────────────────

#[tokio::test]
async fn spawned_agent_routes_message_to_parent() {
    let (hub, _event_rx) = make_hub();

    // Parent agent
    let (parent_conn, parent_rx) = hub_server::connect_local(hub.clone(), "parent");
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    let pc = parent_conn.clone();
    tokio::spawn(async move {
        let mut rx = parent_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = method_tx.send(method).await;
                let _ = pc.respond(id, json!({"ok": true})).await;
            }
        }
    });

    // Mock child agent (registered as if spawned by Hub)
    let (child_client, child_server) = loopal_ipc::duplex_pair();
    let child_conn = Arc::new(Connection::new(child_client));
    let server_conn = Arc::new(Connection::new(child_server));

    let _child_rx = child_conn.start();
    let server_rx = server_conn.start();
    register_agent_connection(hub.clone(), "child", server_conn, server_rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Child sends hub/route targeting parent
    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000002",
        "source": {"Agent": "child"},
        "target": "parent",
        "content": {"text": "report from child", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = child_conn
        .send_request(methods::HUB_ROUTE.name, envelope)
        .await;
    assert!(result.is_ok(), "child should route to parent via Hub");

    // Parent should receive agent/message
    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(
        method.unwrap().unwrap(),
        methods::AGENT_MESSAGE.name,
        "parent should receive agent/message from child"
    );
}
