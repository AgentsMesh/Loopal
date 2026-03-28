//! Advanced multi-agent scenarios: chain routing, permission timeout,
//! recursive spawn, concurrent events.

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

fn envelope(from: &str, target: &str, text: &str) -> serde_json::Value {
    json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "source": {"Agent": from},
        "target": target,
        "content": {"text": text, "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    })
}

// ── Scenario 5: Three-agent chain A→B→C ─────────────────────────────

#[tokio::test]
async fn chain_routing_a_to_b_to_c() {
    let (hub, _) = make_hub();

    // C: capture final message
    let (conn_c, rx_c) = hub_server::connect_local(hub.clone(), "C");
    let (final_tx, mut final_rx) = mpsc::channel::<String>(1);
    let cc = conn_c.clone();
    tokio::spawn(async move {
        let mut rx = rx_c;
        while let Some(Incoming::Request { id, params, .. }) = rx.recv().await {
            let text = params["content"]["text"].as_str().unwrap_or("").to_string();
            let _ = final_tx.send(text).await;
            let _ = cc.respond(id, json!({"ok": true})).await;
        }
    });

    // B: receive from A, forward to C
    let (conn_b, rx_b) = hub_server::connect_local(hub.clone(), "B");
    let cb = conn_b.clone();
    tokio::spawn(async move {
        let mut rx = rx_b;
        while let Some(Incoming::Request { id, params, .. }) = rx.recv().await {
            let text = params["content"]["text"].as_str().unwrap_or("");
            let fwd = envelope("B", "C", &format!("forwarded: {text}"));
            let _ = cb.send_request(methods::HUB_ROUTE.name, fwd).await;
            let _ = cb.respond(id, json!({"ok": true})).await;
        }
    });

    // A: send to B
    let (conn_a, ra) = hub_server::connect_local(hub.clone(), "A");
    tokio::spawn(async move { let mut rx = ra; while rx.recv().await.is_some() {} });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = conn_a
        .send_request(methods::HUB_ROUTE.name, envelope("A", "B", "hello"))
        .await;
    assert!(result.is_ok());

    let text = tokio::time::timeout(Duration::from_secs(2), final_rx.recv()).await;
    assert_eq!(text.unwrap().unwrap(), "forwarded: hello");
}

// ── Scenario 9: Permission denied on TUI disconnect ─────────────────

#[tokio::test]
async fn permission_denied_when_tui_disconnects_mid_request() {
    let (hub, _) = make_hub();

    // TUI connects but will be dropped before responding
    let (_tui_conn, _tui_rx) = hub_server::connect_local(hub.clone(), "_tui");

    let (agent_conn, _) = hub_server::connect_local(hub.clone(), "requester");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Unregister TUI from Hub (simulates TUI crash — Hub detects disconnect)
    hub.lock().await.unregister_connection("_tui");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Agent requests permission — TUI gone, should get deny quickly
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        agent_conn.send_request(
            methods::AGENT_PERMISSION.name,
            json!({"tool_call_id": "t1", "tool_name": "Bash", "tool_input": {}}),
        ),
    )
    .await;

    assert!(result.is_ok(), "should not timeout");
    let resp = result.unwrap().unwrap();
    assert_eq!(resp["allow"], false, "should deny when TUI is gone");
}

// ── Scenario 17: Recursive spawn root→A→B ───────────────────────────

#[tokio::test]
async fn recursive_agent_nesting_grandchild_routes_to_root() {
    let (hub, _) = make_hub();

    // Root: capture messages from grandchild
    let (root_conn, root_rx) = hub_server::connect_local(hub.clone(), "root");
    let (msg_tx, mut msg_rx) = mpsc::channel::<String>(1);
    let rc = root_conn.clone();
    tokio::spawn(async move {
        let mut rx = root_rx;
        while let Some(Incoming::Request { id, params, .. }) = rx.recv().await {
            let text = params["content"]["text"].as_str().unwrap_or("").to_string();
            let _ = msg_tx.send(text).await;
            let _ = rc.respond(id, json!({"ok": true})).await;
        }
    });

    // Child A (registered as sub-agent of root)
    let (t1, t2) = loopal_ipc::duplex_pair();
    let child_a = Arc::new(Connection::new(t1));
    let server_a = Arc::new(Connection::new(t2));
    let _ra = child_a.start();
    let sra = server_a.start();
    register_agent_connection(hub.clone(), "child-a", server_a, sra, None, None).await;

    // Grandchild B (registered as sub-agent of child-a, same Hub)
    let (t3, t4) = loopal_ipc::duplex_pair();
    let grandchild_b = Arc::new(Connection::new(t3));
    let server_b = Arc::new(Connection::new(t4));
    let _rb = grandchild_b.start();
    let srb = server_b.start();
    register_agent_connection(hub.clone(), "grandchild-b", server_b, srb, None, None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Grandchild B sends message to root (skipping parent)
    let result = grandchild_b
        .send_request(
            methods::HUB_ROUTE.name,
            envelope("grandchild-b", "root", "hello from depth 2"),
        )
        .await;
    assert!(result.is_ok());

    let text = tokio::time::timeout(Duration::from_secs(2), msg_rx.recv()).await;
    assert_eq!(text.unwrap().unwrap(), "hello from depth 2");
}

// ── Scenario 20: Concurrent events from multiple agents ─────────────

#[tokio::test]
async fn concurrent_events_all_reach_hub() {
    let (hub, mut event_rx) = make_hub();

    let mut agent_conns = Vec::new();
    for i in 0..3 {
        let name = format!("evt-agent-{i}");
        let (t1, t2) = loopal_ipc::duplex_pair();
        let agent = Arc::new(Connection::new(t1));
        let server = Arc::new(Connection::new(t2));
        let _rx = agent.start();
        let srx = server.start();
        // Register as sub-agent (is_root=false) so agent_name gets tagged
        loopal_agent_hub::agent_io::start_agent_io(
            hub.clone(), &name, server, srx, false,
        );
        agent_conns.push((name, agent));
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    // All 3 agents emit events concurrently
    let mut handles = Vec::new();
    for (name, conn) in &agent_conns {
        let c = conn.clone();
        let n = name.clone();
        handles.push(tokio::spawn(async move {
            let event = json!({
                "agent_name": null,
                "payload": {"Stream": {"text": format!("from {n}")}}
            });
            c.send_notification(methods::AGENT_EVENT.name, event)
                .await
                .unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    // Collect all events
    tokio::time::sleep(Duration::from_millis(200)).await;
    let mut received_names: Vec<String> = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        if let Some(name) = event.agent_name {
            received_names.push(name);
        }
    }

    received_names.sort();
    assert_eq!(
        received_names,
        vec!["evt-agent-0", "evt-agent-1", "evt-agent-2"],
        "all 3 agents' events should reach Hub with correct names"
    );
}
