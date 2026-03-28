//! Tests for completion output capture and topology tracking.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_agent_hub::AgentHub;
use loopal_agent_hub::hub_server;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEvent;
use serde_json::json;

fn make_hub() -> (Arc<Mutex<AgentHub>>, mpsc::Receiver<AgentEvent>) {
    let (tx, rx) = mpsc::channel::<AgentEvent>(64);
    (Arc::new(Mutex::new(AgentHub::new(tx))), rx)
}

/// emit_agent_finished passes actual output (not hardcoded "agent finished").
#[tokio::test]
async fn completion_output_passed_through_wait() {
    let (hub, _) = make_hub();

    // Register agent (keep both sides alive)
    let (_ca, ct) = loopal_ipc::duplex_pair();
    let conn = Arc::new(Connection::new(ct));
    let rx = conn.start();
    register_agent_connection(hub.clone(), "worker", conn, rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Set up waiter
    let hub2 = hub.clone();
    let waiter = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &hub2, methods::HUB_WAIT_AGENT.name,
            json!({"name": "worker"}), "parent".into(),
        ).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Simulate agent completion: emit BEFORE unregister (matches production order)
    {
        let mut h = hub.lock().await;
        h.emit_agent_finished("worker", Some("42 findings in the analysis.".into()));
        h.unregister_connection("worker");
    }

    let result = tokio::time::timeout(Duration::from_secs(3), waiter).await;
    assert!(result.is_ok(), "waiter should resolve");
    let output = result.unwrap().unwrap().unwrap();
    let text = output["output"].as_str().unwrap();
    assert!(
        text.contains("42 findings"),
        "should contain actual output, got: {text}"
    );
}

/// emit_agent_finished with None output falls back to "(no output)".
#[tokio::test]
async fn completion_no_output_fallback() {
    let (hub, _) = make_hub();

    let (_ca, ct) = loopal_ipc::duplex_pair();
    let conn = Arc::new(Connection::new(ct));
    let rx = conn.start();
    register_agent_connection(hub.clone(), "worker2", conn, rx, None, None).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    let hub2 = hub.clone();
    let waiter = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &hub2, methods::HUB_WAIT_AGENT.name,
            json!({"name": "worker2"}), "parent".into(),
        ).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    {
        let mut h = hub.lock().await;
        h.emit_agent_finished("worker2", None);
        h.unregister_connection("worker2");
    }

    let result = tokio::time::timeout(Duration::from_secs(3), waiter).await;
    let output = result.unwrap().unwrap().unwrap();
    let text = output["output"].as_str().unwrap();
    assert_eq!(text, "(no output)");
}

/// Topology tracks parent-child relationships from spawn.
#[tokio::test]
async fn topology_tracks_parent_child() {
    let (hub, _) = make_hub();

    // Register parent (keep both sides of duplex alive!)
    let (_pa, pt) = loopal_ipc::duplex_pair();
    let parent_conn = Arc::new(Connection::new(pt));
    let parent_rx = parent_conn.start();
    register_agent_connection(
        hub.clone(), "parent", parent_conn, parent_rx, None, Some("opus"),
    ).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Register child with parent relationship
    let (_ca, ct) = loopal_ipc::duplex_pair();
    let child_conn = Arc::new(Connection::new(ct));
    let child_rx = child_conn.start();
    register_agent_connection(
        hub.clone(), "child-1", child_conn, child_rx, Some("parent"), Some("sonnet"),
    ).await;
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check topology
    let h = hub.lock().await;
    let parent_info = h.agent_info("parent").expect("parent should exist");
    assert_eq!(parent_info.children, vec!["child-1"]);
    assert_eq!(parent_info.model.as_deref(), Some("opus"));

    let child_info = h.agent_info("child-1").expect("child should exist");
    assert_eq!(child_info.parent.as_deref(), Some("parent"));
    assert_eq!(child_info.model.as_deref(), Some("sonnet"));

    let descendants = h.descendants("parent");
    assert_eq!(descendants, vec!["child-1"]);
}

/// hub/topology returns serializable snapshot.
#[tokio::test]
async fn topology_query_via_hub() {
    let (hub, _) = make_hub();

    let (parent_conn, parent_rx) = hub_server::connect_local(hub.clone(), "requester");
    tokio::spawn(async move {
        let mut rx = parent_rx;
        while let Some(_msg) = rx.recv().await {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = parent_conn
        .send_request(methods::HUB_TOPOLOGY.name, json!({}))
        .await;
    assert!(result.is_ok());
    let data = result.unwrap();
    assert!(data["agents"].is_array());
}
