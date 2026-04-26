//! End-to-end cluster integration tests.
//!
//! Boots a real MetaHub + real Hub instances with real agent processes
//! to verify the complete cross-hub communication stack.

mod cluster_harness;

use std::time::Duration;

use loopal_ipc::protocol::methods;
use serde_json::json;

use cluster_harness::{HubHandle, MetaHubHandle};

/// Two-hub cluster: both agents become ready via real IPC.
#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn cluster_boots_two_hubs_with_agents() {
    let meta = MetaHubHandle::boot().await;
    let hub_a = HubHandle::boot("hub-a", &meta).await;
    let hub_b = HubHandle::boot("hub-b", &meta).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify both hubs registered in MetaHub
    let mh = meta.meta_hub.lock().await;
    assert_eq!(mh.registry.len(), 2, "both hubs should be registered");
    drop(mh);

    // Verify agents are running (have connection in registry)
    assert!(
        hub_a
            .hub
            .lock()
            .await
            .registry
            .get_agent_connection("main")
            .is_some(),
        "hub-a should have main agent"
    );
    assert!(
        hub_b
            .hub
            .lock()
            .await
            .registry
            .get_agent_connection("main")
            .is_some(),
        "hub-b should have main agent"
    );

    // Cleanup
    let _ = hub_a.agent_proc.shutdown().await;
    let _ = hub_b.agent_proc.shutdown().await;
}

/// ListHubs from hub-a sees hub-b (and vice versa) via MetaHub.
#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn cluster_list_hubs_via_agent() {
    let meta = MetaHubHandle::boot().await;
    let hub_a = HubHandle::boot("hub-a", &meta).await;
    let hub_b = HubHandle::boot("hub-b", &meta).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Agent on hub-a queries meta/list_hubs via dispatch
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a.hub,
        methods::META_LIST_HUBS.name,
        json!({}),
        "main".into(),
    )
    .await;

    assert!(result.is_ok(), "list_hubs should succeed: {result:?}");
    let hubs = result.unwrap();
    let names: Vec<&str> = hubs["hubs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|h| h["name"].as_str())
        .collect();
    assert!(names.contains(&"hub-a"), "should see self");
    assert!(names.contains(&"hub-b"), "should see other hub");

    let _ = hub_a.agent_proc.shutdown().await;
    let _ = hub_b.agent_proc.shutdown().await;
}

/// Cross-hub message routing: hub-a sends to hub-b's agent via MetaHub.
#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn cluster_cross_hub_message_delivery() {
    let meta = MetaHubHandle::boot().await;
    let hub_a = HubHandle::boot("hub-a", &meta).await;
    let hub_b = HubHandle::boot("hub-b", &meta).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Route message from hub-a to hub-b/main
    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000020",
        "source": {"Agent": {"hub": [], "agent": "main"}},
        "target": {"hub": ["hub-b"], "agent": "main"},
        "content": {"text": "hello from hub-a", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a.hub,
        methods::HUB_ROUTE.name,
        envelope,
        "main".into(),
    )
    .await;

    assert!(result.is_ok(), "cross-hub route should succeed: {result:?}");

    let _ = hub_a.agent_proc.shutdown().await;
    let _ = hub_b.agent_proc.shutdown().await;
}
