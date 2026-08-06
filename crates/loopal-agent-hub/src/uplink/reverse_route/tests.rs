use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::Envelope;
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

use crate::{Hub, uplink};

#[tokio::test]
async fn blackhole_reverse_route_releases_hub_lock_and_times_out() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (agent_transport, hub_agent_transport) = loopal_ipc::duplex_pair();
    let (_agent, mut agent_rx) = Connection::new(agent_transport).into_listening();
    let (hub_agent, _hub_agent_rx) = Connection::new(hub_agent_transport).into_listening();
    hub.lock()
        .await
        .registry
        .register_connection("target", hub_agent)
        .unwrap();

    let (meta_transport, reverse_transport) = loopal_ipc::duplex_pair();
    let (meta, _meta_rx) = Connection::new(meta_transport).into_listening();
    let (reverse, reverse_rx) = Connection::new(reverse_transport).into_listening();
    let handler = tokio::spawn(uplink::handle_reverse_requests(
        hub.clone(),
        reverse,
        reverse_rx,
        "local-hub".into(),
    ));
    let envelope: Envelope = serde_json::from_value(json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": {"hub": ["origin"], "agent": "sender"}},
        "target": {"hub": [], "agent": "target"},
        "content": {"text": "hello", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    }))
    .unwrap();
    let request = tokio::spawn({
        let meta = meta.clone();
        async move {
            meta.send_request(
                methods::AGENT_MESSAGE.name,
                serde_json::to_value(envelope).unwrap(),
            )
            .await
        }
    });

    assert!(matches!(
        agent_rx.recv().await,
        Some(Incoming::Request { .. })
    ));
    let lock = tokio::time::timeout(Duration::from_millis(20), hub.lock())
        .await
        .expect("reverse route must not hold the Hub mutex");
    drop(lock);
    let response = tokio::time::timeout(Duration::from_secs(1), request)
        .await
        .expect("reverse route must have a deadline")
        .unwrap()
        .unwrap();
    assert_eq!(response["ok"], false);

    meta.close().await;
    handler.await.unwrap();
}
