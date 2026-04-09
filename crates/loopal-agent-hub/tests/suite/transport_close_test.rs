//! Tests that the IO loop closes the transport after agent completion.
//!
//! Verifies the fix for the sub-agent process leak: after receiving
//! `agent/completed`, the Hub must close the transport writer so the
//! child process's blocking stdin read gets EOF and the process can exit.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// After `agent/completed`, the Hub-side transport must be disconnected.
/// This is the critical fix: without `conn.close()`, the child process
/// would hang forever on a blocking stdin read.
#[tokio::test]
async fn transport_closed_after_agent_completes() {
    let (hub, _event_rx) = make_hub();

    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let hub_transport_ref = hub_transport.clone();

    let agent_conn = Arc::new(Connection::new(agent_transport));
    let server_conn = Arc::new(Connection::new(hub_transport));

    let _agent_rx = agent_conn.start();
    let server_rx = server_conn.start();

    register_agent_connection(
        hub.clone(),
        "worker",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Agent sends agent/completed — triggers IO loop exit + conn.close()
    agent_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "end_turn", "result": "done"}),
        )
        .await
        .unwrap();

    // Wait for IO loop to process completion and close transport
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(
        !hub_transport_ref.is_connected(),
        "Hub-side transport must be disconnected after agent/completed"
    );
}

/// The agent (child) side must receive EOF after Hub closes the transport,
/// enabling the child process to exit its reader loop.
#[tokio::test]
async fn agent_receives_eof_after_hub_closes_transport() {
    let (hub, _event_rx) = make_hub();

    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let agent_transport_ref = agent_transport.clone();

    let agent_conn = Arc::new(Connection::new(agent_transport));
    let server_conn = Arc::new(Connection::new(hub_transport));

    let _agent_rx = agent_conn.start();
    let server_rx = server_conn.start();

    register_agent_connection(
        hub.clone(),
        "worker",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Agent sends completion
    agent_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "end_turn", "result": "done"}),
        )
        .await
        .unwrap();

    // Wait for Hub to close the transport
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Agent's reader should now get EOF when trying to read.
    // recv() returns Ok(None) on EOF.
    let recv_result =
        tokio::time::timeout(Duration::from_secs(2), agent_transport_ref.recv()).await;

    match recv_result {
        Ok(Ok(None)) => {} // EOF — correct, Hub closed its writer
        Ok(Ok(Some(_))) => panic!("should not receive data after Hub closed transport"),
        Ok(Err(_)) => {} // read error is also acceptable (broken pipe)
        Err(_) => panic!("agent recv should not timeout — Hub must close transport"),
    }
}

/// Result is fully delivered to the parent before transport close.
#[tokio::test]
async fn result_delivered_before_transport_close() {
    let (hub, _event_rx) = make_hub();

    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let agent_conn = Arc::new(Connection::new(agent_transport));
    let server_conn = Arc::new(Connection::new(hub_transport));

    let _agent_rx = agent_conn.start();
    let server_rx = server_conn.start();

    register_agent_connection(
        hub.clone(),
        "worker",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await;
    // Set up a completion watcher before the agent finishes
    let mut watcher = {
        let mut h = hub.lock().await;
        h.registry.watch_completion("worker")
    };
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Agent sends completion with result
    agent_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "end_turn", "result": "the answer is 42"}),
        )
        .await
        .unwrap();

    // Wait for the watcher to receive the result (set by emit_agent_finished,
    // called in finish_and_deliver BEFORE conn.close)
    let result = tokio::time::timeout(Duration::from_secs(2), watcher.changed()).await;
    assert!(result.is_ok(), "watcher should be notified");
    assert_eq!(
        watcher.borrow().as_deref(),
        Some("the answer is 42"),
        "result must be delivered before transport close"
    );
}

/// When the child process crashes (closes its connection without sending
/// `agent/completed`), the Hub must still close the transport so the
/// `agent_proc.wait()` background task can reap the child.
#[tokio::test]
async fn child_crash_triggers_transport_close() {
    let (hub, _event_rx) = make_hub();

    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let hub_transport_ref = hub_transport.clone();

    let agent_conn = Arc::new(Connection::new(agent_transport));
    let server_conn = Arc::new(Connection::new(hub_transport));

    let _agent_rx = agent_conn.start();
    let server_rx = server_conn.start();

    register_agent_connection(
        hub.clone(),
        "crasher",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Simulate child crash: close the agent-side writer (child's stdout closes).
    // Hub's reader will get EOF → IO loop exits → finish_and_deliver → conn.close.
    agent_conn.close().await;

    // Wait for Hub to detect EOF and close its side
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert!(
        !hub_transport_ref.is_connected(),
        "Hub must close transport even when child crashes without agent/completed"
    );

    // Agent should be unregistered from Hub
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("crasher")
            .is_none(),
        "crashed agent must be unregistered"
    );
}

/// After completion and transport close, the agent must no longer be
/// routable in the Hub registry.
#[tokio::test]
async fn agent_unregistered_after_completion() {
    let (hub, _event_rx) = make_hub();

    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let agent_conn = Arc::new(Connection::new(agent_transport));
    let server_conn = Arc::new(Connection::new(hub_transport));

    let _agent_rx = agent_conn.start();
    let server_rx = server_conn.start();

    register_agent_connection(
        hub.clone(),
        "ephemeral",
        server_conn,
        server_rx,
        None,
        None,
        None,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify registered before completion
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("ephemeral")
            .is_some(),
        "agent should be registered before completion"
    );

    agent_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "end_turn", "result": "ok"}),
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(200)).await;

    // Must be unregistered after completion
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("ephemeral")
            .is_none(),
        "agent must be unregistered after completion"
    );
}
