use std::sync::Arc;

use loopal_agent_hub::{Hub, agent_io, hub_server};
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_ipc::{RpcError, TcpTransport};
use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, mpsc};

#[tokio::test]
async fn internal_string_cannot_create_agent_principal() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    for method in [
        methods::HUB_SPAWN_AGENT.name,
        methods::HUB_SPAWN_REMOTE_AGENT.name,
    ] {
        let error = loopal_agent_hub::dispatch::dispatch_hub_request(
            &hub,
            method,
            json!({"name": "child"}),
            "main".into(),
        )
        .await
        .unwrap_err();
        assert!(
            error.contains("principal") || error.contains("MetaHub"),
            "{error}"
        );
    }
}

#[tokio::test]
async fn managed_agent_reaches_local_validation_with_typed_principal() {
    let root = tempfile::tempdir().unwrap();
    let (event_tx, mut event_rx) = mpsc::channel(16);
    let hub = Arc::new(Mutex::new(Hub::with_cwd(event_tx, root.path().into())));
    let (agent_transport, hub_transport) = loopal_ipc::duplex_pair();
    let (agent, mut agent_rx) = Connection::new(agent_transport).into_listening();
    let (hub_connection, hub_rx) = Connection::new(hub_transport).into_listening();
    let dispatcher = Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
        hub.clone(),
    ));
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    agent_io::start_agent_io(
        hub,
        dispatcher,
        loopal_protocol::ROOT_AGENT_NAME,
        hub_connection,
        hub_rx,
        Some(ready_tx),
    );
    ready_rx.await.unwrap();
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let reverse_agent = agent.clone();
    tokio::spawn(async move {
        while let Some(incoming) = agent_rx.recv().await {
            if let loopal_ipc::connection::Incoming::Request { id, .. } = incoming {
                let _ = reverse_agent.respond(id, json!({"ok": true})).await;
            }
        }
    });

    let error = agent
        .send_request(
            methods::HUB_SPAWN_AGENT.name,
            json!({"name": "child", "depth": 0}),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("depth"), "{error}");
}

#[tokio::test]
async fn external_tcp_agent_cannot_spawn() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (listener, port, token) = hub_server::start_hub_listener(hub.clone()).await.unwrap();
    tokio::spawn(hub_server::accept_loop(listener, hub, token.clone()));
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (client, _incoming) = Connection::new(transport).into_listening();
    client
        .send_request(
            methods::HUB_REGISTER.name,
            json!({"name": "external", "token": token, "role": "agent"}),
        )
        .await
        .unwrap();

    let error = client
        .send_request(methods::HUB_SPAWN_AGENT.name, json!({"name": "child"}))
        .await
        .unwrap_err();
    assert!(
        matches!(error, RpcError::Remote { ref message, .. } if message.contains("not authorized")),
        "{error}"
    );
}
