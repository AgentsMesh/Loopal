//! Integration tests for Agent collaboration tool actions via Hub.
//! Tests: spawn+result chain, status query, cascade shutdown, SendMessage.

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

/// Full chain: spawn agent → agent completes → wait_agent returns real output.
/// Simulates the Agent(action=spawn) + Agent(action=result) flow.
#[tokio::test]
async fn spawn_and_result_full_chain() {
    let (hub, _) = make_hub();

    // Parent connects to Hub
    let (parent_conn, parent_rx) = hub_server::connect_local(hub.clone(), "parent");
    tokio::spawn(async move {
        let mut rx = parent_rx;
        while let Some(_msg) = rx.recv().await {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Register mock child (simulates hub/spawn_agent result)
    let (_ca, ct) = loopal_ipc::duplex_pair();
    let child = Arc::new(Connection::new(ct));
    let child_rx = child.start();
    register_agent_connection(
        hub.clone(),
        "worker",
        child,
        child_rx,
        Some("parent"),
        Some("sonnet"),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Parent calls hub/wait_agent (like Agent action=result)
    let pc = parent_conn.clone();
    let waiter = tokio::spawn(async move {
        pc.send_request(methods::HUB_WAIT_AGENT.name, json!({"name": "worker"}))
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Worker completes with real output
    {
        let mut h = hub.lock().await;
        h.emit_agent_finished("worker", Some("Analysis: 42 crates found.".into()));
        h.unregister_connection("worker");
    }

    let result = tokio::time::timeout(Duration::from_secs(3), waiter).await;
    let output = result.unwrap().unwrap().unwrap();
    assert!(
        output["output"].as_str().unwrap().contains("42 crates"),
        "should get real output: {output:?}"
    );
}

/// Agent(action=status) for running and finished agents.
#[tokio::test]
async fn agent_info_running_and_finished() {
    let (hub, _) = make_hub();

    let (parent_conn, parent_rx) = hub_server::connect_local(hub.clone(), "querier");
    tokio::spawn(async move {
        let mut rx = parent_rx;
        while let Some(_msg) = rx.recv().await {}
    });

    // Register child
    let (_ca, ct) = loopal_ipc::duplex_pair();
    let child = Arc::new(Connection::new(ct));
    let child_rx = child.start();
    register_agent_connection(
        hub.clone(),
        "child-a",
        child,
        child_rx,
        Some("querier"),
        Some("opus"),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Query running agent
    let info = parent_conn
        .send_request(methods::HUB_AGENT_INFO.name, json!({"name": "child-a"}))
        .await
        .unwrap();
    assert_eq!(info["lifecycle"].as_str().unwrap(), "Running");
    assert_eq!(info["model"].as_str().unwrap(), "opus");
    assert_eq!(info["parent"].as_str().unwrap(), "querier");

    // Finish agent
    {
        let mut h = hub.lock().await;
        h.emit_agent_finished("child-a", Some("done!".into()));
        h.unregister_connection("child-a");
    }

    // Query finished agent — should find cached output
    let info2 = parent_conn
        .send_request(methods::HUB_AGENT_INFO.name, json!({"name": "child-a"}))
        .await
        .unwrap();
    assert_eq!(info2["lifecycle"].as_str().unwrap(), "Finished");
    assert_eq!(info2["output"].as_str().unwrap(), "done!");
}

/// Query nonexistent agent returns error.
#[tokio::test]
async fn agent_info_not_found() {
    let (hub, _) = make_hub();
    let (conn, rx) = hub_server::connect_local(hub.clone(), "q");
    tokio::spawn(async move {
        let mut rx = rx;
        while let Some(_msg) = rx.recv().await {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = conn
        .send_request(methods::HUB_AGENT_INFO.name, json!({"name": "ghost"}))
        .await;
    // Should get error response (not transport error)
    assert!(result.is_ok());
    let val = result.unwrap();
    assert!(val.get("code").is_some() || val.get("message").is_some());
}

/// SendMessage (hub/route) to running agent succeeds; to finished agent fails.
#[tokio::test]
async fn send_message_running_vs_finished() {
    let (hub, _) = make_hub();

    // Sender
    let (sender, sr) = hub_server::connect_local(hub.clone(), "sender");
    tokio::spawn(async move {
        let mut rx = sr;
        while let Some(_msg) = rx.recv().await {}
    });

    // Receiver (responds to agent/message)
    let (recv_conn, recv_rx) = hub_server::connect_local(hub.clone(), "receiver");
    let rc = recv_conn.clone();
    tokio::spawn(async move {
        let mut rx = recv_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, .. } = msg {
                let _ = rc.respond(id, json!({"ok": true})).await;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Route to running agent → should succeed
    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": "sender"},
        "target": "receiver",
        "content": {"text": "hello", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = sender.send_request(methods::HUB_ROUTE.name, envelope).await;
    assert!(result.is_ok(), "route to running agent should succeed");

    // Unregister receiver (simulating agent exit)
    {
        let mut h = hub.lock().await;
        h.emit_agent_finished("receiver", None);
        h.unregister_connection("receiver");
    }

    // Route to finished agent → should fail
    let envelope2 = json!({
        "id": "00000000-0000-0000-0000-000000000002",
        "source": {"Agent": "sender"},
        "target": "receiver",
        "content": {"text": "hello again", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result2 = sender
        .send_request(methods::HUB_ROUTE.name, envelope2)
        .await;
    assert!(result2.is_ok());
    let val = result2.unwrap();
    assert!(
        val.get("code").is_some() || val.get("message").is_some(),
        "route to finished agent should return error"
    );
}

/// Cascade shutdown: parent finishes → children get interrupted.
#[tokio::test]
async fn cascade_shutdown_interrupts_children() {
    let (hub, mut event_rx) = make_hub();

    // Register parent
    let (_pa, pt) = loopal_ipc::duplex_pair();
    let parent = Arc::new(Connection::new(pt));
    let parent_rx = parent.start();
    register_agent_connection(hub.clone(), "parent", parent, parent_rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Register child with interrupt capture
    let (child_client, child_server) = loopal_ipc::duplex_pair();
    let child_conn = Arc::new(Connection::new(child_client));
    let server_conn = Arc::new(Connection::new(child_server));
    let client_rx = child_conn.start();
    let server_rx = server_conn.start();
    register_agent_connection(
        hub.clone(),
        "child",
        server_conn,
        server_rx,
        Some("parent"),
        None,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Listen for interrupts on child's client side
    let (interrupt_tx, mut interrupt_rx) = mpsc::channel::<bool>(1);
    let cc = child_conn.clone();
    tokio::spawn(async move {
        let mut rx = client_rx;
        while let Some(msg) = rx.recv().await {
            if let Incoming::Notification { method, .. } = &msg {
                if method == methods::AGENT_INTERRUPT.name {
                    let _ = interrupt_tx.send(true).await;
                }
            }
            if let Incoming::Request { id, .. } = msg {
                let _ = cc.respond(id, json!({"ok": true})).await;
            }
        }
    });

    // Parent finishes → should cascade interrupt to child
    {
        let mut h = hub.lock().await;
        h.emit_agent_finished("parent", Some("parent done".into()));
        h.unregister_connection("parent");
    }

    // Child should receive interrupt
    let got_interrupt = tokio::time::timeout(Duration::from_secs(2), interrupt_rx.recv()).await;
    assert!(
        got_interrupt.is_ok() && got_interrupt.unwrap() == Some(true),
        "child should receive interrupt when parent finishes"
    );

    // Drain events to confirm Finished was emitted for parent
    let mut got_finished = false;
    while let Ok(event) = event_rx.try_recv() {
        if let AgentEventPayload::Finished = event.payload {
            got_finished = true;
        }
    }
    assert!(got_finished, "should emit Finished event for parent");
}
