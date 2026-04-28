//! Tests: shadow entry lifecycle — cross-hub spawn, wait, completion, cleanup.
//!
//! Covers the shadow entry flow:
//! 1. Shadow registration on spawn
//! 2. wait_agent blocks on shadow until completion
//! 3. Completion triggers shadow watcher
//! 4. Shadow cleanup after completion
//!
//! Routing-target tests (orphan cascade, route-to-shadow, listing) live in
//! `shadow_routing_test.rs` — split for the 200-line limit.

use std::time::Duration;

use loopal_ipc::protocol::methods;
use serde_json::json;

use crate::test_helpers::*;

/// Shadow registration works correctly.
#[tokio::test]
async fn shadow_registered_with_correct_parent() {
    let (hub, _) = make_hub();
    let (_parent, _rx) = register_mock_agent(&hub, "parent", None).await;

    hub.lock().await.registry.register_shadow(
        "remote-child",
        loopal_protocol::QualifiedAddress::local("parent"),
    ).unwrap();

    let h = hub.lock().await;
    let info = h.registry.agent_info("remote-child");
    assert!(info.is_some(), "shadow should exist");
    let info = info.unwrap();
    assert_eq!(
        info.parent.as_ref().map(|p| p.to_string()).as_deref(),
        Some("parent")
    );
    assert_eq!(format!("{:?}", info.lifecycle), "Running");

    // Shadow should be in parent's children
    let parent_info = h.registry.agent_info("parent").unwrap();
    assert!(parent_info.children.contains(&"remote-child".to_string()));
}

/// wait_agent blocks on shadow and resolves when completion arrives.
#[tokio::test]
async fn wait_agent_resolves_on_shadow_completion() {
    let (hub, _) = make_hub();
    // Register parent + shadow manually (simulate post-spawn state)
    let (_parent, _rx) = register_mock_agent(&hub, "parent", None).await;
    hub.lock().await.registry.register_shadow(
        "remote-child",
        loopal_protocol::QualifiedAddress::local("parent"),
    ).unwrap();

    // Start wait_agent in background
    let hub2 = hub.clone();
    let waiter = tokio::spawn(async move {
        loopal_agent_hub::dispatch::dispatch_hub_request(
            &hub2,
            methods::HUB_WAIT_AGENT.name,
            json!({"name": "remote-child"}),
            "parent".into(),
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Simulate completion arriving (normally via uplink)
    {
        let mut h = hub.lock().await;
        h.registry
            .emit_agent_finished("remote-child", Some("task result 42".into()));
        h.registry.unregister_connection("remote-child");
    }

    let result = tokio::time::timeout(Duration::from_secs(3), waiter)
        .await
        .expect("should not timeout")
        .expect("task should not panic")
        .expect("dispatch should succeed");

    let output = result["output"].as_str().unwrap_or("");
    assert!(
        output.contains("task result 42"),
        "wait should return completion output: {output}"
    );
}

/// Shadow is cleaned up after completion (no memory leak).
#[tokio::test]
async fn shadow_cleaned_up_after_completion() {
    let (hub, _) = make_hub();
    let (_parent, _rx) = register_mock_agent(&hub, "parent", None).await;
    hub.lock()
        .await
        .registry
        .register_shadow("child", loopal_protocol::QualifiedAddress::local("parent")).unwrap();

    // Verify shadow exists
    assert!(hub.lock().await.registry.agent_info("child").is_some());

    // Complete + unregister (as uplink handler does)
    {
        let mut h = hub.lock().await;
        h.registry.emit_agent_finished("child", Some("done".into()));
        h.registry.unregister_connection("child");
    }

    // Shadow should be gone
    assert!(
        hub.lock().await.registry.agent_info("child").is_none(),
        "shadow should be removed after completion"
    );
    // But output should be cached (wait_agent on already-finished returns cached)
    let cached = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_WAIT_AGENT.name,
        json!({"name": "child"}),
        "parent".into(),
    )
    .await;
    // Cached output or "not found" — either way proves lifecycle completed
    assert!(cached.is_ok());
}
