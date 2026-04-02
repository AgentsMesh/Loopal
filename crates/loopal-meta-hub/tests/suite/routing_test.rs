//! Tests: cross-hub message routing (hub/route → uplink → MetaHub → target Hub).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::protocol::methods;
use serde_json::json;

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

#[tokio::test]
async fn route_message_across_hubs() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_b, _) = make_hub();
    let _conn_b = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    let (_agent_conn, _rx) = register_mock_agent(&hub_b, "target", None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": "sender"}, "target": "target",
        "content": {"text": "cross-hub hello", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta_hub,
        methods::META_ROUTE.name,
        envelope,
        "hub-a".into(),
    )
    .await;
    assert!(result.is_ok(), "route should succeed: {result:?}");
}

#[tokio::test]
async fn hub_uplink_escalates_unknown_agent() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let _hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    let (_agent_conn, _rx) = register_mock_agent(&hub_b, "remote-worker", None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000002",
        "source": {"Agent": "local"}, "target": "remote-worker",
        "content": {"text": "find you", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_ROUTE.name,
        envelope,
        "local".into(),
    )
    .await;
    assert!(
        result.is_ok(),
        "uplink escalation should succeed: {result:?}"
    );
}

#[tokio::test]
async fn qualified_address_routes_via_uplink() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let _hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    let (_agent_conn, _rx) = register_mock_agent(&hub_b, "worker", None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000003",
        "source": {"Agent": "sender"}, "target": "hub-b/worker",
        "content": {"text": "explicit hub target", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_ROUTE.name,
        envelope,
        "sender".into(),
    )
    .await;
    assert!(result.is_ok(), "qualified route should succeed: {result:?}");
}

/// Route to agent that doesn't exist on any Hub returns error.
#[tokio::test]
async fn route_to_nonexistent_agent_fails() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000004",
        "source": {"Agent": "sender"}, "target": "ghost-agent",
        "content": {"text": "nobody home", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_ROUTE.name,
        envelope,
        "sender".into(),
    )
    .await;
    // Uplink now propagates MetaHub error responses as Err()
    assert!(
        result.is_err(),
        "route to nonexistent agent should fail: {result:?}"
    );
}

/// Route to agent on disconnected Hub fails after cleanup.
#[tokio::test]
async fn route_after_hub_disconnect_fails() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hub_a, _) = make_hub();
    let (hub_b, _) = make_hub();

    let hub_a_conn = wire_hub_to_meta("hub-a", &hub_a, &meta_hub).await;
    let _hub_b_conn = wire_hub_to_meta("hub-b", &hub_b, &meta_hub).await;
    {
        let ul = Arc::new(loopal_agent_hub::HubUplink::new(hub_a_conn, "hub-a".into()));
        hub_a.lock().await.uplink = Some(ul);
    }
    let (_agent, _rx) = register_mock_agent(&hub_b, "target", None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    meta_hub.lock().await.remove_hub("hub-b");

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000005",
        "source": {"Agent": "sender"}, "target": "target",
        "content": {"text": "too late", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a,
        methods::HUB_ROUTE.name,
        envelope,
        "sender".into(),
    )
    .await;
    assert!(
        result.is_err(),
        "route after hub disconnect should fail: {result:?}"
    );
}
