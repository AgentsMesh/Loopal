use std::sync::Arc;

use loopal_agent_hub::{Hub, hub_server};
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_ipc::{RpcError, TcpTransport};
use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};

#[tokio::test]
async fn tcp_agent_cannot_forge_reserved_meta_principal() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (listener, port, token) = hub_server::start_hub_listener(hub.clone()).await.unwrap();
    tokio::spawn(hub_server::accept_loop(
        listener,
        hub.clone(),
        token.clone(),
    ));
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (conn, _incoming) = Connection::new(transport).into_listening();

    let error = conn
        .send_request(
            methods::HUB_REGISTER.name,
            json!({
                "name": "meta:forged",
                "token": token,
                "role": "agent",
            }),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        RpcError::Remote { message, .. } if message.contains("reserved principal prefix")
    ));
    assert!(
        hub.lock()
            .await
            .registry
            .get_agent_connection("meta:forged")
            .is_none()
    );
}

#[tokio::test]
async fn string_prefix_never_authorizes_remote_relay_dispatch() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let error = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::HUB_REMOTE_RELAY.name,
        json!({"operation": "interrupt", "payload": {"target": "worker"}}),
        "meta:forged".into(),
    )
    .await
    .unwrap_err();

    assert!(error.contains("authenticated reverse MetaHub transport"));
}
