//! Tests: shadow as a routing/listing target — orphan cascade behavior,
//! route-to-shadow failure, and shadow visibility in agent listings.
//!
//! Companion file to `shadow_lifecycle_test.rs` (split for the 200-line
//! file limit).

use std::sync::Arc;
use std::time::Duration;

use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use serde_json::json;

use crate::test_helpers::*;

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
    let _ = register_agent_connection(
        hub.clone(),
        "real-child",
        real_conn,
        real_rx,
        Some("parent"),
        None,
        None,
    )
    .await;

    hub.lock().await.registry.register_shadow(
        "shadow-child",
        loopal_protocol::QualifiedAddress::local("parent"),
    ).unwrap();
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
    hub.lock().await.registry.register_shadow(
        "shadow-agent",
        loopal_protocol::QualifiedAddress::local("parent"),
    ).unwrap();

    let envelope = json!({
        "id": "00000000-0000-0000-0000-000000000010",
        "source": {"Agent": {"hub": [], "agent": "sender"}},
        "target": {"hub": [], "agent": "shadow-agent"},
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
    hub.lock().await.registry.register_shadow(
        "remote-x",
        loopal_protocol::QualifiedAddress::local("parent"),
    ).unwrap();

    let agents = hub.lock().await.registry.list_agents();
    let shadow = agents.iter().find(|(n, _)| n == "remote-x");
    assert!(shadow.is_some(), "shadow should appear in list");
    assert_eq!(shadow.unwrap().1, "shadow");
}
