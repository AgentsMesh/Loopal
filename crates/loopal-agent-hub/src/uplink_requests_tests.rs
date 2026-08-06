use loopal_ipc::connection::{Connection, Incoming};
use loopal_protocol::Envelope;
use serde_json::json;

use crate::HubUplink;

#[tokio::test(start_paused = true)]
async fn route_and_spawn_requests_are_bounded_against_blackhole_meta_hub() {
    let (client_transport, peer_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (_peer, mut peer_rx) = Connection::new(peer_transport).into_listening();
    let peer = tokio::spawn(async move {
        for _ in 0..3 {
            loop {
                match peer_rx.recv().await {
                    Some(Incoming::Request { .. }) => break,
                    Some(Incoming::Notification { .. }) => continue,
                    None => panic!("peer connection closed before all requests arrived"),
                }
            }
        }
    });
    let uplink = HubUplink::new(client.clone(), "hub-a".into());
    let envelope: Envelope = serde_json::from_value(json!({
        "id": "00000000-0000-0000-0000-000000000001",
        "source": {"Agent": {"hub": [], "agent": "sender"}},
        "target": {"hub": ["hub-b"], "agent": "target"},
        "content": {"text": "hello", "images": []},
        "timestamp": "2026-01-01T00:00:00Z"
    }))
    .unwrap();

    assert!(
        uplink
            .route(&envelope)
            .await
            .unwrap_err()
            .contains("timed out")
    );
    assert!(
        uplink
            .spawn_agent(json!({"target_hub": "hub-b"}))
            .await
            .unwrap_err()
            .contains("timed out")
    );
    assert!(
        uplink
            .relay_remote(json!({"target_hub": "hub-b", "operation": "interrupt"}))
            .await
            .unwrap_err()
            .contains("timed out")
    );
    client.close().await;
    peer.await.unwrap();
}
