use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, mpsc};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_meta_hub::MetaHub;
use serde_json::json;

#[tokio::test(start_paused = true)]
async fn blackhole_forwarding_is_bounded_without_holding_registry_lock() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (meta_transport, peer_transport) = loopal_ipc::duplex_pair();
    let (meta_conn, _meta_rx) = Connection::new(meta_transport).into_listening();
    let (_peer, mut peer_rx) = Connection::new(peer_transport).into_listening();
    meta_hub
        .lock()
        .await
        .registry
        .register("hub-b", meta_conn, vec![])
        .unwrap();
    let (seen_tx, mut seen_rx) = mpsc::channel(2);
    let peer = tokio::spawn(async move {
        let mut requests = 0;
        while requests < 2 {
            match peer_rx.recv().await {
                Some(Incoming::Request { .. }) => {
                    requests += 1;
                    seen_tx.send(()).await.unwrap();
                }
                Some(Incoming::Notification { method, .. })
                    if method == methods::REQUEST_CANCEL.name => {}
                other => panic!("unexpected blackhole peer message: {other:?}"),
            }
        }
    });

    let route_hub = meta_hub.clone();
    let route = tokio::spawn(async move {
        loopal_meta_hub::dispatch::dispatch_meta_request(
            &route_hub,
            methods::META_ROUTE.name,
            json!({
                "id": "00000000-0000-0000-0000-000000000001",
                "source": {"Agent": {"hub": [], "agent": "sender"}},
                "target": {"hub": ["hub-b"], "agent": "target"},
                "content": {"text": "hello", "images": []},
                "timestamp": "2026-01-01T00:00:00Z"
            }),
            "hub-a".into(),
        )
        .await
    });
    seen_rx.recv().await.unwrap();
    assert!(
        meta_hub.try_lock().is_ok(),
        "route held MetaHub mutex across IO"
    );
    tokio::time::advance(Duration::from_secs(6)).await;
    assert!(route.await.unwrap().unwrap_err().contains("timed out"));

    let spawn_hub = meta_hub.clone();
    let spawn = tokio::spawn(async move {
        loopal_meta_hub::dispatch::dispatch_meta_request(
            &spawn_hub,
            methods::META_SPAWN.name,
            json!({"name": "child", "prompt": "work", "target_hub": "hub-b"}),
            "hub-a".into(),
        )
        .await
    });
    seen_rx.recv().await.unwrap();
    assert!(
        meta_hub.try_lock().is_ok(),
        "spawn held MetaHub mutex across IO"
    );
    tokio::time::advance(Duration::from_secs(31)).await;
    assert!(spawn.await.unwrap().unwrap_err().contains("timed out"));
    peer.await.unwrap();
}
