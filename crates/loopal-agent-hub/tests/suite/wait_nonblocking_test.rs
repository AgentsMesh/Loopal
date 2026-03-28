//! Regression test: hub/wait_agent must NOT block the agent IO loop.
//!
//! When an agent sends hub/wait_agent, subsequent hub/* requests from the
//! same agent must still be processed. This tests the fix where wait_agent
//! is spawned as a background task instead of being awaited inline.

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

/// Regression: agent sends hub/wait_agent for child-A, then hub/wait_agent for child-B.
/// Both waits must proceed concurrently; the first must not block the second.
#[tokio::test]
async fn wait_agent_does_not_block_io_loop() {
    let (hub, _event_rx) = make_hub();

    // Register two mock child agents
    let (_child_a_conn, child_a_server, child_a_rx) = mock_agent();
    register_agent_connection(hub.clone(), "child-a", child_a_server, child_a_rx, None, None).await;

    let (_child_b_conn, child_b_server, child_b_rx) = mock_agent();
    register_agent_connection(hub.clone(), "child-b", child_b_server, child_b_rx, None, None).await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Parent agent connects via local connection (goes through agent_io_loop)
    let (parent_conn, parent_rx) = hub_server::connect_local(hub.clone(), "parent");
    // Drain incoming notifications/requests from parent's rx (not needed here)
    tokio::spawn(async move {
        let mut rx = parent_rx;
        while let Some(_msg) = rx.recv().await {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Parent sends hub/wait_agent for child-a (should NOT block IO loop)
    let pc1 = parent_conn.clone();
    let wait_a = tokio::spawn(async move {
        pc1.send_request(methods::HUB_WAIT_AGENT.name, json!({"name": "child-a"}))
            .await
    });

    // Give IO loop time to process the first wait
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Parent sends hub/wait_agent for child-b (would be stuck if IO loop blocked)
    let pc2 = parent_conn.clone();
    let wait_b = tokio::spawn(async move {
        pc2.send_request(methods::HUB_WAIT_AGENT.name, json!({"name": "child-b"}))
            .await
    });

    // Complete child-b first (to prove both waits are independent)
    tokio::time::sleep(Duration::from_millis(100)).await;
    {
        let mut h = hub.lock().await;
        h.emit_agent_finished("child-b", Some("result-b".into()));
    }

    // wait_b should resolve quickly
    let result_b = tokio::time::timeout(Duration::from_secs(2), wait_b).await;
    assert!(
        result_b.is_ok(),
        "wait_b should complete after child-b finishes (not blocked by wait_a)"
    );

    // Now complete child-a
    {
        let mut h = hub.lock().await;
        h.emit_agent_finished("child-a", Some("result-a".into()));
    }

    let result_a = tokio::time::timeout(Duration::from_secs(2), wait_a).await;
    assert!(result_a.is_ok(), "wait_a should complete after child-a finishes");
}

/// After hub/wait_agent is pending, hub/spawn_agent from the same agent must still work.
#[tokio::test]
async fn spawn_after_wait_not_blocked() {
    let (hub, _event_rx) = make_hub();

    // Register a mock child that we'll wait on
    let (_child_conn, child_server, child_rx) = mock_agent();
    register_agent_connection(hub.clone(), "existing-child", child_server, child_rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Parent connects through IO loop
    let (parent_conn, parent_rx) = hub_server::connect_local(hub.clone(), "parent");
    tokio::spawn(async move {
        let mut rx = parent_rx;
        while let Some(_msg) = rx.recv().await {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Parent sends hub/wait_agent (will block until child finishes)
    let pc1 = parent_conn.clone();
    let _wait_handle = tokio::spawn(async move {
        pc1.send_request(
            methods::HUB_WAIT_AGENT.name,
            json!({"name": "existing-child"}),
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Parent sends hub/list_agents — should NOT be blocked
    let pc2 = parent_conn.clone();
    let list_result =
        tokio::time::timeout(Duration::from_secs(2), async move {
            pc2.send_request(methods::HUB_LIST_AGENTS.name, json!({}))
                .await
        })
        .await;

    assert!(
        list_result.is_ok(),
        "hub/list_agents should not be blocked by pending wait_agent"
    );
    let agents = list_result.unwrap().unwrap();
    assert!(agents["agents"].is_array());
}

fn mock_agent() -> (Arc<Connection>, Arc<Connection>, mpsc::Receiver<Incoming>) {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let client_conn = Arc::new(Connection::new(client_transport));
    let server_conn = Arc::new(Connection::new(server_transport));
    let _client_rx = client_conn.start();
    let server_rx = server_conn.start();
    // Wrap server_rx in a channel so register_agent_connection can consume it
    let (tx, rx) = mpsc::channel(256);
    tokio::spawn(async move {
        let mut srx = server_rx;
        while let Some(msg) = srx.recv().await {
            if tx.send(msg).await.is_err() { break; }
        }
    });
    (client_conn, server_conn, rx)
}
