//! Multi-agent interaction scenarios: concurrent permissions, disconnection,
//! sibling communication, chained routing, wait edge cases.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::AgentHub;
use loopal_agent_hub::hub_server;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<AgentHub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(AgentHub::new(tx))), rx)
}

fn spawn_mock(conn: Arc<Connection>, rx: mpsc::Receiver<Incoming>) {
    tokio::spawn(async move {
        let mut rx = rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = conn.respond(id, json!({"ok": true})).await;
            }
        }
    });
}

// ── Concurrent permissions from multiple agents ─────────────────────

#[tokio::test]
async fn concurrent_permissions_from_two_agents() {
    let (hub, _) = make_hub();

    let (tui_conn, tui_rx) = hub_server::connect_local(hub.clone(), "_tui");
    let tc = tui_conn.clone();
    tokio::spawn(async move {
        let mut rx = tui_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = tc.respond(id, json!({"allow": true})).await;
            }
        }
    });

    let (conn_a, _) = hub_server::connect_local(hub.clone(), "agent-a");
    let (conn_b, _) = hub_server::connect_local(hub.clone(), "agent-b");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Both agents request permission concurrently
    let a_handle = {
        let c = conn_a.clone();
        tokio::spawn(async move {
            c.send_request(methods::AGENT_PERMISSION.name, json!({"tool_call_id": "a1", "tool_name": "Bash", "tool_input": {}})).await
        })
    };
    let b_handle = {
        let c = conn_b.clone();
        tokio::spawn(async move {
            c.send_request(methods::AGENT_PERMISSION.name, json!({"tool_call_id": "b1", "tool_name": "Edit", "tool_input": {}})).await
        })
    };

    let (a_res, b_res) = tokio::join!(a_handle, b_handle);
    assert_eq!(a_res.unwrap().unwrap()["allow"], true);
    assert_eq!(b_res.unwrap().unwrap()["allow"], true);
}

// ── Route to disconnected agent ─────────────────────────────────────

#[tokio::test]
async fn route_to_disconnected_agent_returns_error() {
    let (hub, _) = make_hub();

    // Register then immediately unregister (simulate disconnect)
    {
        let (_t1, t2) = loopal_ipc::duplex_pair();
        let conn = Arc::new(Connection::new(t2));
        let _rx = conn.start();
        let mut h = hub.lock().await;
        let _ = h.register_connection("ghost", conn);
        h.unregister_connection("ghost");
    }

    let (sender, sr) = hub_server::connect_local(hub.clone(), "sender");
    spawn_mock(sender.clone(), sr);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000003",
        "source": {"Agent": "sender"},
        "target": "ghost",
        "content": {"text": "are you there?", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = sender
        .send_request(methods::HUB_ROUTE.name, envelope)
        .await;
    // Should get error response (ghost is gone)
    assert!(result.is_ok());
    let val = result.unwrap();
    assert!(val.get("code").is_some() || val.get("message").is_some());
}

// ── Two siblings communicate directly ───────────────────────────────

#[tokio::test]
async fn sibling_agents_communicate_via_hub() {
    let (hub, _) = make_hub();

    let (conn_b, rx_b) = hub_server::connect_local(hub.clone(), "sibling-b");
    let (method_tx, mut method_rx) = mpsc::channel::<String>(1);
    let cb = conn_b.clone();
    tokio::spawn(async move {
        let mut rx = rx_b;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, .. } = msg {
                let _ = method_tx.send(method).await;
                let _ = cb.respond(id, json!({"ok": true})).await;
            }
        }
    });

    let (conn_c, rc) = hub_server::connect_local(hub.clone(), "sibling-c");
    spawn_mock(conn_c.clone(), rc);
    tokio::time::sleep(Duration::from_millis(50)).await;

    // C sends to B directly (siblings, not parent-child)
    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000004",
        "source": {"Agent": "sibling-c"},
        "target": "sibling-b",
        "content": {"text": "hey sibling", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = conn_c.send_request(methods::HUB_ROUTE.name, envelope).await;
    assert!(result.is_ok());

    let method = tokio::time::timeout(Duration::from_secs(2), method_rx.recv()).await;
    assert_eq!(method.unwrap().unwrap(), methods::AGENT_MESSAGE.name);
}

// ── Wait for already-finished agent ─────────────────────────────────

#[tokio::test]
async fn wait_already_finished_agent_returns_immediately() {
    let (hub, _) = make_hub();

    // Register and immediately finish
    let (_t1, t2) = loopal_ipc::duplex_pair();
    let conn = Arc::new(Connection::new(t2));
    let rx = conn.start();
    register_agent_connection(hub.clone(), "done-agent", conn, rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Finish it
    hub.lock().await.unregister_connection("done-agent");
    hub.lock().await.emit_agent_finished("done-agent", None);

    // Now wait — should return immediately
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &hub,
            methods::HUB_WAIT_AGENT.name,
            json!({"name": "done-agent"}),
            "waiter".into(),
        ),
    )
    .await;
    assert!(result.is_ok(), "should return immediately for finished agent");
    assert!(result.unwrap().is_ok());
}

// ── Multiple waiters on same agent ──────────────────────────────────

#[tokio::test]
async fn multiple_waiters_on_same_agent() {
    let (hub, _) = make_hub();

    let (_t1, t2) = loopal_ipc::duplex_pair();
    let conn = Arc::new(Connection::new(t2));
    let rx = conn.start();
    register_agent_connection(hub.clone(), "shared-target", conn, rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Two waiters
    let h1 = hub.clone();
    let w1 = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &h1, methods::HUB_WAIT_AGENT.name,
            json!({"name": "shared-target"}), "w1".into(),
        ).await
    });
    let h2 = hub.clone();
    let w2 = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &h2, methods::HUB_WAIT_AGENT.name,
            json!({"name": "shared-target"}), "w2".into(),
        ).await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    hub.lock().await.emit_agent_finished("shared-target", None);

    let (r1, r2) = tokio::join!(
        tokio::time::timeout(Duration::from_secs(2), w1),
        tokio::time::timeout(Duration::from_secs(2), w2),
    );
    assert!(r1.is_ok(), "waiter 1 should complete");
    assert!(r2.is_ok(), "waiter 2 should complete");
}

// ── Root agent event has agent_name=None ─────────────────────────────

#[tokio::test]
async fn root_agent_event_preserves_none_agent_name() {
    let (hub, mut event_rx) = make_hub();

    // Register as root (is_root=true)
    let (t1, t2) = loopal_ipc::duplex_pair();
    let agent_conn = Arc::new(Connection::new(t1));
    let server_conn = Arc::new(Connection::new(t2));
    let _agent_rx = agent_conn.start();
    let server_rx = server_conn.start();
    loopal_agent_hub::agent_io::start_agent_io(
        hub.clone(), "main", server_conn, server_rx, true,
    );
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Root agent sends event with agent_name=None
    let event = json!({"agent_name": null, "payload": {"Stream": {"text": "hi"}}});
    agent_conn
        .send_notification(methods::AGENT_EVENT.name, event)
        .await
        .unwrap();

    let received = tokio::time::timeout(Duration::from_secs(2), event_rx.recv()).await;
    let evt = received.unwrap().unwrap();
    assert!(
        evt.agent_name.is_none(),
        "root agent event should keep agent_name=None, got {:?}",
        evt.agent_name
    );
}
