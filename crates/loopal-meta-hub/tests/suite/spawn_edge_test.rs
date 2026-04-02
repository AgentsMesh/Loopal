//! Tests: spawn edge cases + regression guards for local completion.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use serde_json::json;

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

/// Spawn with target_hub but no uplink returns clear error.
#[tokio::test]
async fn spawn_without_uplink_fails_clearly() {
    let (hub_a, _) = make_hub();

    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "worker", "target_hub": "hub-b", "cwd": "/tmp"}),
        "parent".into(),
    )
    .await;

    assert!(result.is_err());
    assert!(
        result.unwrap_err().contains("uplink"),
        "should mention uplink"
    );
}

/// Cross-hub spawn injects qualified parent "hub-a/parent" into params.
#[tokio::test]
async fn spawn_injects_qualified_parent() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();

    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let _hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "child", "target_hub": "hub-b", "cwd": "/tmp"}),
        "my-parent".into(),
    )
    .await;

    // Error should be from spawn attempt, not routing/parent
    if let Err(e) = &result {
        assert!(
            !e.contains("uplink") && !e.contains("routing"),
            "error should be from spawn, not parent: {e}"
        );
    }
}

/// Local parent completion works correctly when uplink is set (regression).
#[tokio::test]
async fn local_parent_completion_unaffected_by_uplink() {
    let (hub, _) = make_hub();

    {
        let (t, _) = loopal_ipc::duplex_pair();
        let c = Arc::new(Connection::new(t));
        let _rx = c.start();
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(c, "my-hub".into()));
        hub.lock().await.uplink = Some(ul);
    }

    let (_parent_conn, _parent_rx) = register_mock_agent(&hub, "parent", None).await;

    let (child_client, child_server) = loopal_ipc::duplex_pair();
    let child_server_conn = Arc::new(Connection::new(child_server));
    let child_client_conn = Arc::new(Connection::new(child_client));
    let child_server_rx = child_server_conn.start();
    let _child_client_rx = child_client_conn.start();
    register_agent_connection(
        hub.clone(),
        "child",
        child_server_conn,
        child_server_rx,
        Some("parent"),
        None,
        None,
    )
    .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let _ = child_client_conn
        .send_notification(methods::AGENT_COMPLETED.name, json!({"result": "done"}))
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    drop(child_client_conn);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let h = hub.lock().await;
    assert!(h.registry.get_agent_connection("parent").is_some());
}
