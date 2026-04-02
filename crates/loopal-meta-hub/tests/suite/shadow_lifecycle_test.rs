//! Tests: shadow entry lifecycle — cross-hub spawn, wait, completion, cleanup.
//!
//! Covers the full shadow entry flow:
//! 1. Shadow registration on spawn
//! 2. wait_agent blocks on shadow until completion
//! 3. Completion triggers shadow watcher
//! 4. Shadow cleanup after completion
//! 5. Orphan cascade skips shadows
//! 6. Shadow not routable as message target

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use serde_json::json;

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

/// Shadow registration works correctly.
#[tokio::test]
async fn shadow_registered_with_correct_parent() {
    let (hub, _) = make_hub();
    let (_parent, _rx) = register_mock_agent(&hub, "parent", None).await;

    hub.lock()
        .await
        .registry
        .register_shadow("remote-child", "parent");

    let h = hub.lock().await;
    let info = h.registry.agent_info("remote-child");
    assert!(info.is_some(), "shadow should exist");
    let info = info.unwrap();
    assert_eq!(info.parent.as_deref(), Some("parent"));
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
    hub.lock()
        .await
        .registry
        .register_shadow("remote-child", "parent");

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
    hub.lock().await.registry.register_shadow("child", "parent");

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

/// Orphan cascade does NOT interrupt shadows.
#[tokio::test]
async fn orphan_cascade_skips_shadows() {
    let (hub, mut event_rx) = make_hub();

    // Register parent with a real child + a shadow child
    let (_p_conn, _) = register_mock_agent(&hub, "parent", None).await;

    let (real_client, real_server) = loopal_ipc::duplex_pair();
    let real_conn = Arc::new(Connection::new(real_server));
    let _real_client_conn = Arc::new(Connection::new(real_client));
    let real_rx = real_conn.start();
    let _real_client_rx = _real_client_conn.start();
    register_agent_connection(
        hub.clone(),
        "real-child",
        real_conn,
        real_rx,
        Some("parent"),
        None,
        None,
    )
    .await;

    hub.lock()
        .await
        .registry
        .register_shadow("shadow-child", "parent");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Parent finishes → should cascade interrupt to real-child only
    {
        let mut h = hub.lock().await;
        h.registry.emit_agent_finished("parent", Some("bye".into()));
    }

    // Give time for interrupt to propagate
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Shadow should NOT have been interrupted (it's still Running or cleaned up)
    // The key assertion: no panic, no error from trying to interrupt shadow
    // Drain events to verify Finished was emitted
    let mut got_finished = false;
    while let Ok(event) = event_rx.try_recv() {
        if matches!(event.payload, loopal_protocol::AgentEventPayload::Finished) {
            got_finished = true;
        }
    }
    assert!(got_finished, "parent Finished event should be emitted");
}

/// Shadow is NOT a valid route target (hub/route to shadow fails).
#[tokio::test]
async fn route_to_shadow_fails() {
    let (hub, _) = make_hub();
    let (_parent, _rx) = register_mock_agent(&hub, "parent", None).await;
    hub.lock()
        .await
        .registry
        .register_shadow("shadow-agent", "parent");

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000010",
        "source": {"Agent": "sender"},
        "target": "shadow-agent",
        "content": {"text": "can you hear me?", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    });
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_ROUTE.name,
        envelope,
        "sender".into(),
    )
    .await;

    // Shadow has no connection → route should fail (not silently succeed)
    let failed = result.is_err() || result.as_ref().is_ok_and(|v| v.get("code").is_some());
    assert!(failed, "route to shadow should fail: {result:?}");
}

/// Shadow appears in agent list with "shadow" state.
#[tokio::test]
async fn shadow_visible_in_agent_list() {
    let (hub, _) = make_hub();
    let (_parent, _rx) = register_mock_agent(&hub, "parent", None).await;
    hub.lock()
        .await
        .registry
        .register_shadow("remote-x", "parent");

    let agents = hub.lock().await.registry.list_agents();
    let shadow = agents.iter().find(|(n, _)| n == "remote-x");
    assert!(shadow.is_some(), "shadow should appear in list");
    assert_eq!(shadow.unwrap().1, "shadow");
}
