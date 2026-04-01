//! Tests for parallel sub-agent spawning and result collection.
//! Uses real agent_io_loop path: agent sends agent/completed → Hub extracts result.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::Hub;
use loopal_agent_hub::hub_server;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<Hub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(Hub::new(tx))), rx)
}

/// Helper: register an agent, returning the client-side connection.
/// The client connection must stay alive until the agent "completes".
async fn register_agent(
    hub: &Arc<Mutex<Hub>>,
    name: &str,
    parent: Option<&str>,
) -> Arc<Connection> {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let client_conn = Arc::new(Connection::new(client_transport));
    let server_conn = Arc::new(Connection::new(server_transport));
    let _client_rx = client_conn.start();
    let server_rx = server_conn.start();
    register_agent_connection(
        hub.clone(),
        name,
        server_conn,
        server_rx,
        parent,
        None,
        None,
    )
    .await;
    client_conn
}

/// Two agents completing in reverse order — both results collected correctly.
#[tokio::test]
async fn parallel_agents_reverse_completion_order() {
    let (hub, _) = make_hub();

    let (parent_conn, parent_rx) = hub_server::connect_local(hub.clone(), "parent");
    tokio::spawn(async move {
        let mut rx = parent_rx;
        while let Some(_msg) = rx.recv().await {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Keep client connections alive (drop triggers EOF → premature completion)
    let agent_a = register_agent(&hub, "agent-a", Some("parent")).await;
    let agent_b = register_agent(&hub, "agent-b", Some("parent")).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Parent waits for both concurrently
    let pc_a = parent_conn.clone();
    let wait_a = tokio::spawn(async move {
        pc_a.send_request(methods::HUB_WAIT_AGENT.name, json!({"name": "agent-a"}))
            .await
    });
    let pc_b = parent_conn.clone();
    let wait_b = tokio::spawn(async move {
        pc_b.send_request(methods::HUB_WAIT_AGENT.name, json!({"name": "agent-b"}))
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Agent-b completes first via agent/completed notification
    agent_b
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "goal", "result": "result-B"}),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Agent-a completes second
    agent_a
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "goal", "result": "result-A"}),
        )
        .await
        .unwrap();

    let out_a = tokio::time::timeout(Duration::from_secs(3), wait_a)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let out_b = tokio::time::timeout(Duration::from_secs(3), wait_b)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(out_a["output"].as_str().unwrap(), "result-A");
    assert_eq!(out_b["output"].as_str().unwrap(), "result-B");
}

/// Three agents with mixed results: success, empty, and no result field.
#[tokio::test]
async fn parallel_agents_mixed_results() {
    let (hub, _) = make_hub();
    let (parent_conn, parent_rx) = hub_server::connect_local(hub.clone(), "parent");
    tokio::spawn(async move {
        let mut rx = parent_rx;
        while let Some(_msg) = rx.recv().await {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let ok_conn = register_agent(&hub, "ok-agent", Some("parent")).await;
    let empty_conn = register_agent(&hub, "empty-agent", Some("parent")).await;
    let none_conn = register_agent(&hub, "none-agent", Some("parent")).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let pc1 = parent_conn.clone();
    let w1 = tokio::spawn(async move {
        pc1.send_request(methods::HUB_WAIT_AGENT.name, json!({"name": "ok-agent"}))
            .await
    });
    let pc2 = parent_conn.clone();
    let w2 = tokio::spawn(async move {
        pc2.send_request(methods::HUB_WAIT_AGENT.name, json!({"name": "empty-agent"}))
            .await
    });
    let pc3 = parent_conn.clone();
    let w3 = tokio::spawn(async move {
        pc3.send_request(methods::HUB_WAIT_AGENT.name, json!({"name": "none-agent"}))
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Complete all three with different results
    ok_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "goal", "result": "detailed report"}),
        )
        .await
        .unwrap();
    empty_conn
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "goal", "result": ""}),
        )
        .await
        .unwrap();
    // No result field at all (legacy/error case)
    none_conn
        .send_notification(methods::AGENT_COMPLETED.name, json!({"reason": "shutdown"}))
        .await
        .unwrap();

    let r1 = tokio::time::timeout(Duration::from_secs(3), w1)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let r2 = tokio::time::timeout(Duration::from_secs(3), w2)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let r3 = tokio::time::timeout(Duration::from_secs(3), w3)
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    assert_eq!(r1["output"].as_str().unwrap(), "detailed report");
    assert_eq!(r2["output"].as_str().unwrap(), "");
    assert_eq!(r3["output"].as_str().unwrap(), "(no output)");
}
