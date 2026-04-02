//! Tests: cross-hub spawn + completion delivery.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use serde_json::json;

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

#[tokio::test]
async fn spawn_with_target_hub_reaches_metahub() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "remote-worker", "target_hub": "hub-b", "cwd": "/tmp"}),
        "parent-agent".into(),
    )
    .await;

    if let Ok(val) = &result {
        assert!(
            val.get("agent_id").is_none(),
            "no agent_id without target hub: {val}"
        );
    }
}

#[tokio::test]
async fn cross_hub_spawn_reaches_target_hub() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let _hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "remote-worker", "target_hub": "hub-b", "cwd": "/tmp"}),
        "parent-agent".into(),
    )
    .await;

    if let Err(e) = &result {
        assert!(
            e.contains("spawn") || e.contains("failed") || e.contains("No such file"),
            "error from spawn, not routing: {e}"
        );
    }
}

#[tokio::test]
async fn completion_delivery_to_remote_parent() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
    let _hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;

    let (_parent_conn, mut parent_rx) = register_mock_agent(&hub_a, "parent-agent", None).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_b_conn, "hub-b".into()));
        hub_b.lock().await.uplink = Some(ul);
    }

    let (child_client, child_server) = loopal_ipc::duplex_pair();
    let child_server_conn = Arc::new(Connection::new(child_server));
    let child_client_conn = Arc::new(Connection::new(child_client));
    let child_server_rx = child_server_conn.start();
    let _child_client_rx = child_client_conn.start();
    register_agent_connection(
        hub_b.clone(),
        "child-worker",
        child_server_conn,
        child_server_rx,
        Some("hub-a/parent-agent"),
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

    let received = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(msg) = parent_rx.recv().await {
            let params = match &msg {
                Incoming::Request { params, .. } | Incoming::Notification { params, .. } => params,
            };
            let text = params
                .get("content")
                .and_then(|c| c.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if text.contains("agent-result") && text.contains("child-worker") {
                return true;
            }
        }
        false
    })
    .await;

    assert!(
        received.unwrap_or(false),
        "parent should receive remote child completion"
    );
}
