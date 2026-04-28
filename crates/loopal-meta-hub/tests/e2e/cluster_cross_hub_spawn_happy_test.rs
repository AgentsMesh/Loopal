//! Happy-path e2e: full chain hub-a → MetaHub → hub-b spawn → response.
//! Verifies shadow registration, real-subprocess spawn, and duplicate-name
//! rejection at the receiver-side.

use std::time::Duration;

use loopal_ipc::protocol::methods;
use serde_json::json;

use crate::cluster_harness::{HubHandle, MetaHubHandle};

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn cluster_cross_hub_spawn_happy_path_reaches_receiver() {
    let meta = MetaHubHandle::boot().await;
    let hub_a = HubHandle::boot("hub-a", &meta).await;
    let hub_b = HubHandle::boot("hub-b", &meta).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a.hub,
        methods::HUB_SPAWN_AGENT.name,
        json!({
            "name": "remote-child",
            "prompt": "say hi",
            "target_hub": "hub-b",
        }),
        "main".into(),
    )
    .await;

    let resp = result.expect("cross-hub spawn should succeed");
    assert_eq!(resp["name"].as_str(), Some("remote-child"));
    assert!(
        resp["agent_id"].as_str().is_some_and(|s| !s.is_empty()),
        "agent_id must be present, got: {resp}"
    );

    {
        let h = hub_a.hub.lock().await;
        assert!(
            h.registry.agent_info("remote-child").is_some(),
            "hub-a must have shadow"
        );
    }
    {
        let h = hub_b.hub.lock().await;
        assert!(
            h.registry.agent_info("remote-child").is_some(),
            "hub-b must have child"
        );
    }

    let _ = hub_a.agent_proc.shutdown().await;
    let _ = hub_b.agent_proc.shutdown().await;
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn cluster_cross_hub_spawn_rejects_duplicate_name() {
    let meta = MetaHubHandle::boot().await;
    let hub_a = HubHandle::boot("hub-a", &meta).await;
    let hub_b = HubHandle::boot("hub-b", &meta).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    let first = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a.hub,
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "dup", "prompt": "hi", "target_hub": "hub-b"}),
        "main".into(),
    )
    .await;
    assert!(first.is_ok(), "first spawn: {first:?}");

    let second = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a.hub,
        methods::HUB_SPAWN_AGENT.name,
        json!({"name": "dup", "prompt": "again", "target_hub": "hub-b"}),
        "main".into(),
    )
    .await;
    let err = second.expect_err("duplicate name must be rejected");
    assert!(
        err.contains("already registered") || err.contains("dup"),
        "got: {err}"
    );

    let _ = hub_a.agent_proc.shutdown().await;
    let _ = hub_b.agent_proc.shutdown().await;
}
