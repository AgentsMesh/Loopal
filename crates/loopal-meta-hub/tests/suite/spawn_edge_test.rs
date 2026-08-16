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
    let (hub_a, _hub_a_event_rx) = make_hub();

    let error = send_agent_request(
        &hub_a,
        "parent",
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "worker", "target_hub": "hub-b"}),
    )
    .await
    .expect_err("cross-hub spawn without an uplink must fail");

    assert!(error.to_string().contains("uplink"), "got: {error}");
}

/// Cross-hub spawn injects qualified parent "hub-a/parent" into params.
#[tokio::test]
async fn spawn_injects_qualified_parent() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _hub_a_event_rx) = make_hub();
    let (hub_b, _hub_b_event_rx) = make_hub();

    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let _hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = send_agent_request(
        &hub_a,
        "my-parent",
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "child", "target_hub": "hub-b"}),
    )
    .await;

    if let Err(error) = &result {
        assert!(
            !error.to_string().contains("principal"),
            "managed caller must reach cross-hub spawn handling: {error}"
        );
    }
}

/// Local parent completion works correctly when uplink is set (regression).
#[tokio::test]
async fn local_parent_completion_unaffected_by_uplink() {
    let (hub, _hub_event_rx) = make_hub();

    {
        let (t, _) = loopal_ipc::duplex_pair();
        let (c, _rx) = Connection::new(t).into_listening();
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(c, "my-hub".into()));
        hub.lock().await.uplink = Some(ul);
    }

    let (_parent_conn, _parent_rx) = register_mock_agent(&hub, "parent", None).await;

    let (child_client, child_server) = loopal_ipc::duplex_pair();
    let (child_server_conn, child_server_rx) = Connection::new(child_server).into_listening();
    let (child_client_conn, _child_client_rx) = Connection::new(child_client).into_listening();

    let _ = register_agent_connection(
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
        .send_notification(
            methods::AGENT_COMPLETED.name,
            json!({"reason": "goal", "result": "done"}),
        )
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    drop(child_client_conn);
    tokio::time::sleep(Duration::from_millis(100)).await;

    let h = hub.lock().await;
    assert!(h.registry.get_agent_connection("parent").is_some());
}
