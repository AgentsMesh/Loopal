use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use loopal_ipc::RpcError;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::protocol::methods;
use loopal_meta_hub::MetaHub;
use serde_json::json;

async fn start_source(
    meta_hub: &Arc<Mutex<MetaHub>>,
    name: &str,
) -> (Arc<Connection<Listening>>, tokio::task::JoinHandle<()>) {
    let (client_transport, meta_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (meta_conn, meta_rx) = Connection::new(meta_transport).into_listening();
    meta_hub
        .lock()
        .await
        .registry
        .register(name, meta_conn.clone(), vec![])
        .unwrap();
    let io_hub = meta_hub.clone();
    let io_name = name.to_string();
    let io = tokio::spawn(async move {
        loopal_meta_hub::io_loop::meta_hub_io_loop(io_hub, meta_conn, meta_rx, io_name).await;
    });
    (client, io)
}

async fn register_blackhole(
    meta_hub: &Arc<Mutex<MetaHub>>,
    name: &str,
) -> tokio::sync::mpsc::Receiver<Incoming> {
    let (meta_transport, peer_transport) = loopal_ipc::duplex_pair();
    let (meta_conn, _meta_rx) = Connection::new(meta_transport).into_listening();
    let (_peer, peer_rx) = Connection::new(peer_transport).into_listening();
    meta_hub
        .lock()
        .await
        .registry
        .register(name, meta_conn, vec![])
        .unwrap();
    peer_rx
}

fn relay_params(sequence: usize) -> serde_json::Value {
    json!({
        "target_hub": "target",
        "operation": "control",
        "agent_name": "worker",
        "sequence": sequence,
    })
}

#[tokio::test]
async fn cancelled_blackhole_relay_releases_saturated_slot_while_heartbeat_stays_live() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (source, io) = start_source(&meta_hub, "source").await;
    let mut target_rx = register_blackhole(&meta_hub, "target").await;

    let mut relays = Vec::new();
    for sequence in 0..loopal_meta_hub::io_loop::META_DATA_REQUEST_LIMIT {
        let client = source.clone();
        relays.push(tokio::spawn(async move {
            client
                .send_request(methods::META_REMOTE_RELAY.name, relay_params(sequence))
                .await
        }));
    }
    for _ in 0..loopal_meta_hub::io_loop::META_DATA_REQUEST_LIMIT {
        let message = tokio::time::timeout(Duration::from_secs(1), target_rx.recv())
            .await
            .expect("remote relay was not forwarded")
            .expect("target connection closed");
        assert!(matches!(
            message,
            Incoming::Request { method, .. } if method == methods::HUB_REMOTE_RELAY.name
        ));
    }

    let heartbeat = tokio::time::timeout(
        Duration::from_millis(500),
        source.send_request(methods::META_HEARTBEAT.name, json!({"agent_count": 23})),
    )
    .await
    .expect("heartbeat was blocked by remote relay handlers")
    .expect("heartbeat failed");
    assert_eq!(heartbeat["ok"], true);
    assert_eq!(
        meta_hub
            .lock()
            .await
            .registry
            .snapshot()
            .into_iter()
            .find(|hub| hub.name == "source")
            .unwrap()
            .agent_count,
        23,
    );

    let overflow = source
        .send_request(methods::META_LIST_HUBS.name, json!({}))
        .await
        .unwrap_err();
    assert!(matches!(overflow, RpcError::Remote { code: -32000, .. }));

    let cancelled = relays.pop().unwrap();
    cancelled.abort();
    assert!(cancelled.await.unwrap_err().is_cancelled());
    loop {
        let message = tokio::time::timeout(Duration::from_secs(1), target_rx.recv())
            .await
            .expect("cancel did not reach blackhole target")
            .expect("target connection closed");
        if matches!(
            message,
            Incoming::Notification { method, .. } if method == methods::REQUEST_CANCEL.name
        ) {
            break;
        }
    }

    let listed = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            match source
                .send_request(methods::META_LIST_HUBS.name, json!({}))
                .await
            {
                Ok(value) => break value,
                Err(RpcError::Remote { code: -32000, .. }) => tokio::task::yield_now().await,
                Err(error) => panic!("unexpected request failure after cancellation: {error}"),
            }
        }
    })
    .await
    .expect("cancelled relay did not release its request slot");
    assert_eq!(listed["hubs"].as_array().unwrap().len(), 2);

    for relay in relays {
        relay.abort();
        let _ = relay.await;
    }
    source.close().await;
    tokio::time::timeout(Duration::from_secs(1), io)
        .await
        .expect("MetaHub IO loop did not stop")
        .unwrap();
}

#[tokio::test]
async fn disconnect_aborts_forwarding_and_cleans_only_its_connection_lease() {
    let meta_hub = Arc::new(Mutex::new(MetaHub::new()));
    let (source, io) = start_source(&meta_hub, "source").await;
    let mut target_rx = register_blackhole(&meta_hub, "target").await;
    let relay_source = source.clone();
    let relay = tokio::spawn(async move {
        relay_source
            .send_request(methods::META_REMOTE_RELAY.name, relay_params(0))
            .await
    });
    assert!(matches!(
        target_rx.recv().await,
        Some(Incoming::Request { method, .. }) if method == methods::HUB_REMOTE_RELAY.name
    ));

    source.close().await;
    loop {
        let message = tokio::time::timeout(Duration::from_secs(1), target_rx.recv())
            .await
            .expect("disconnect did not cancel forwarded request")
            .expect("target connection closed");
        if matches!(
            message,
            Incoming::Notification { method, .. } if method == methods::REQUEST_CANCEL.name
        ) {
            break;
        }
    }
    tokio::time::timeout(Duration::from_secs(1), io)
        .await
        .expect("MetaHub IO loop did not stop on disconnect")
        .unwrap();
    assert!(relay.await.unwrap().is_err());
    assert_eq!(meta_hub.lock().await.registry.hub_names(), vec!["target"]);
}
