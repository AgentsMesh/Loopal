use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::tcp::TcpTransport;
use loopal_protocol::AgentEvent;

use super::{connect, disconnect};
use crate::{Hub, HubUplink};

fn test_hub() -> Arc<Mutex<Hub>> {
    let (events, _) = mpsc::channel::<AgentEvent>(8);
    Arc::new(Mutex::new(Hub::new(events)))
}

#[tokio::test(start_paused = true)]
async fn register_blackhole_times_out_without_installing_uplink() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let transport: Arc<dyn loopal_ipc::transport::Transport> =
            Arc::new(TcpTransport::new(stream));
        let (_conn, mut rx) = Connection::new(transport).into_listening();
        assert!(matches!(rx.recv().await, Some(Incoming::Request { .. })));
        while rx.recv().await.is_some() {}
    });
    let hub = test_hub();

    let error = connect(&hub, &address, "secret", "desktop")
        .await
        .unwrap_err();

    assert!(error.contains("register timed out"));
    assert!(hub.lock().await.uplink.is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn heartbeat_blackhole_cleans_installed_uplink_by_identity() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let transport: Arc<dyn loopal_ipc::transport::Transport> =
            Arc::new(TcpTransport::new(stream));
        let (conn, mut rx) = Connection::new(transport).into_listening();
        let Some(Incoming::Request { id, .. }) = rx.recv().await else {
            panic!("missing register request");
        };
        conn.respond(id, serde_json::json!({"ok": true}))
            .await
            .unwrap();
        assert!(matches!(rx.recv().await, Some(Incoming::Request { .. })));
        while rx.recv().await.is_some() {}
    });
    let hub = test_hub();

    let error = connect(&hub, &address, "secret", "desktop")
        .await
        .unwrap_err();

    assert!(error.contains("heartbeat timed out"));
    assert!(hub.lock().await.uplink.is_none());
    server.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn leave_blackhole_is_bounded_and_always_closes_connection() {
    let (client_transport, peer_transport) = loopal_ipc::duplex_pair();
    let (client, _client_rx) = Connection::new(client_transport).into_listening();
    let (_peer, mut peer_rx) = Connection::new(peer_transport).into_listening();
    let hub = test_hub();
    hub.lock().await.uplink = Some(Arc::new(HubUplink::new(client.clone(), "desktop".into())));
    let peer = tokio::spawn(async move {
        assert!(matches!(
            peer_rx.recv().await,
            Some(Incoming::Request { .. })
        ));
        while peer_rx.recv().await.is_some() {}
    });

    disconnect(&hub).await.unwrap();

    assert!(hub.lock().await.uplink.is_none());
    assert!(!client.is_connected());
    peer.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn periodic_heartbeat_blackhole_removes_and_closes_current_uplink() {
    let (client_transport, peer_transport) = loopal_ipc::duplex_pair();
    let (client, client_rx) = Connection::new(client_transport).into_listening();
    let (_peer, mut peer_rx) = Connection::new(peer_transport).into_listening();
    let hub = test_hub();
    let uplink = Arc::new(HubUplink::new(client.clone(), "desktop".into()));
    hub.lock().await.uplink = Some(uplink.clone());
    crate::uplink_tasks::start(hub.clone(), uplink, client_rx);
    let peer = tokio::spawn(async move {
        assert!(matches!(
            peer_rx.recv().await,
            Some(Incoming::Request { .. })
        ));
        while peer_rx.recv().await.is_some() {}
    });

    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(15)).await;
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(6)).await;
    tokio::task::yield_now().await;

    assert!(hub.lock().await.uplink.is_none());
    assert!(!client.is_connected());
    peer.await.unwrap();
}
