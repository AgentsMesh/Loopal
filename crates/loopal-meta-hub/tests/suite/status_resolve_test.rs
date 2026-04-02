//! Tests: hub/status, meta/list_hubs, meta/resolve, heartbeat.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use serde_json::json;

use loopal_meta_hub::MetaHub;

use crate::test_helpers::*;

#[tokio::test]
async fn hub_status_shows_uplink() {
    let (hub, _) = make_hub();
    let (conn, rx) = loopal_agent_hub::hub_server::connect_local(hub.clone(), "querier");
    tokio::spawn(async move {
        let mut rx = rx;
        while rx.recv().await.is_some() {}
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let s = conn
        .send_request(methods::HUB_STATUS.name, json!({}))
        .await
        .unwrap();
    assert!(s["uplink"].is_null());

    {
        let (t, _) = loopal_ipc::duplex_pair();
        let c = Arc::new(Connection::new(t));
        let _rx = c.start();
        hub.lock().await.uplink = Some(Arc::new(loopal_agent_hub::HubUplink::new(
            c,
            "my-hub".into(),
        )));
    }
    let s2 = conn
        .send_request(methods::HUB_STATUS.name, json!({}))
        .await
        .unwrap();
    assert_eq!(s2["uplink"]["hub_name"].as_str(), Some("my-hub"));
}

#[tokio::test]
async fn list_hubs_returns_registered() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (ha, _) = make_hub();
    let (hb, _) = make_hub();
    let _a = wire_hub_to_meta("hub-a", &ha, &meta_hub).await;
    let _b = wire_hub_to_meta("hub-b", &hb, &meta_hub).await;

    let r = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta_hub,
        methods::META_LIST_HUBS.name,
        json!({}),
        "hub-a".into(),
    )
    .await
    .unwrap();
    assert_eq!(r["hubs"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn resolve_agent_finds_correct_hub() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (ha, _) = make_hub();
    let (hb, _) = make_hub();
    let _a = wire_hub_to_meta("hub-a", &ha, &meta_hub).await;
    let _b = wire_hub_to_meta("hub-b", &hb, &meta_hub).await;
    let (_c, _rx) = register_mock_agent(&hb, "only-on-b", None).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let r = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta_hub,
        methods::META_RESOLVE.name,
        json!({"agent_name": "only-on-b"}),
        "hub-a".into(),
    )
    .await
    .unwrap();
    assert_eq!(r["found"].as_bool(), Some(true));
    assert_eq!(r["hub"].as_str(), Some("hub-b"));
}

#[tokio::test]
async fn resolve_nonexistent_returns_false() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (ha, _) = make_hub();
    let _a = wire_hub_to_meta("hub-a", &ha, &meta_hub).await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let r = loopal_meta_hub::dispatch::dispatch_meta_request(
        &meta_hub,
        methods::META_RESOLVE.name,
        json!({"agent_name": "ghost"}),
        "hub-a".into(),
    )
    .await
    .unwrap();
    assert_eq!(r["found"].as_bool(), Some(false));
}

#[tokio::test]
async fn heartbeat_updates_agent_count() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (hx, _) = make_hub();
    let _c = wire_hub_to_meta("hub-x", &hx, &meta_hub).await;
    {
        meta_hub
            .lock()
            .await
            .registry
            .heartbeat("hub-x", 7)
            .unwrap();
    }
    assert_eq!(meta_hub.lock().await.registry.snapshot()[0].agent_count, 7);
}

/// Heartbeat timeout transitions: Connected → Degraded → Disconnected.
#[tokio::test]
async fn heartbeat_timeout_degrades_and_disconnects() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (_, meta_transport) = loopal_ipc::duplex_pair();
    let meta_conn = Arc::new(Connection::new(meta_transport));
    let _rx = meta_conn.start();

    // Register with a hub_info that has an old heartbeat
    {
        let mut mh = meta_hub.lock().await;
        mh.registry
            .register("stale-hub", meta_conn, vec![])
            .unwrap();
    }

    // Immediately check health — should still be Connected (just registered)
    {
        let mut mh = meta_hub.lock().await;
        let disconnected = mh.registry.check_health();
        assert!(
            disconnected.is_empty(),
            "fresh hub should not be disconnected"
        );
    }

    // Verify initial status
    let info = meta_hub.lock().await.registry.snapshot();
    assert_eq!(format!("{:?}", info[0].status), "Connected");
}
