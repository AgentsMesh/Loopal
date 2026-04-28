//! End-to-end tests for cross-hub `spawn_remote_agent` over real TCP +
//! MetaHub forwarding. Schema-rejection paths only — happy-path tests
//! live in `cluster_cross_hub_spawn_happy_test.rs`.

use std::time::Duration;

use loopal_ipc::protocol::methods;
use serde_json::json;

use crate::cluster_harness::{HubHandle, MetaHubHandle};

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn cluster_cross_hub_spawn_rejects_cwd_field() {
    let meta = MetaHubHandle::boot().await;
    let hub_a = HubHandle::boot("hub-a", &meta).await;
    let hub_b = HubHandle::boot("hub-b", &meta).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a.hub,
        methods::HUB_SPAWN_AGENT.name,
        json!({
            "name": "child",
            "prompt": "report your cwd",
            "target_hub": "hub-b",
            "cwd": "/attacker/path",
        }),
        "main".into(),
    )
    .await;

    let err = result.expect_err("MetaHub must reject cwd in cross-hub spawn");
    assert!(err.contains("cwd"), "got: {err}");

    let _ = hub_a.agent_proc.shutdown().await;
    let _ = hub_b.agent_proc.shutdown().await;
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn cluster_cross_hub_spawn_rejects_fork_context() {
    let meta = MetaHubHandle::boot().await;
    let hub_a = HubHandle::boot("hub-a", &meta).await;
    let hub_b = HubHandle::boot("hub-b", &meta).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a.hub,
        methods::HUB_SPAWN_AGENT.name,
        json!({
            "name": "child",
            "prompt": "do work",
            "target_hub": "hub-b",
            "fork_context": [],
        }),
        "main".into(),
    )
    .await;

    let err = result.expect_err("must reject fork_context");
    assert!(err.contains("fork_context"), "got: {err}");

    let _ = hub_a.agent_proc.shutdown().await;
    let _ = hub_b.agent_proc.shutdown().await;
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn cluster_cross_hub_spawn_validates_required_name() {
    let meta = MetaHubHandle::boot().await;
    let hub_a = HubHandle::boot("hub-a", &meta).await;
    let hub_b = HubHandle::boot("hub-b", &meta).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub_a.hub,
        methods::HUB_SPAWN_AGENT.name,
        json!({
            "prompt": "do work",
            "target_hub": "hub-b",
        }),
        "main".into(),
    )
    .await;

    let err = result.expect_err("missing name must surface");
    assert!(err.contains("name"), "got: {err}");

    let _ = hub_a.agent_proc.shutdown().await;
    let _ = hub_b.agent_proc.shutdown().await;
}
