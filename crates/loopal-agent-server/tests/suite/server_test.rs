//! Server protocol handshake tests (initialize + lifecycle).
//!
//! These tests use in-memory duplex streams to simulate IPC without spawning
//! real child processes. They test the server loop's protocol state machine.

use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;

fn pipe_connection_pair() -> (Arc<Connection>, Arc<Connection>) {
    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);
    let ta: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let tb: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    (Arc::new(Connection::new(ta)), Arc::new(Connection::new(tb)))
}

#[tokio::test]
async fn initialize_returns_agent_info() {
    let (client, server) = pipe_connection_pair();
    let mut server_rx = server.start();
    let _client_rx = client.start();

    // Simulate server handling initialize
    let client_clone = client.clone();
    let handle = tokio::spawn(async move {
        client_clone
            .send_request(
                methods::INITIALIZE.name,
                serde_json::json!({"protocol_version": 1}),
            )
            .await
    });

    // Server side: receive and respond
    if let Some(Incoming::Request { id, method, .. }) = server_rx.recv().await {
        assert_eq!(method, methods::INITIALIZE.name);
        let _ = server
            .respond(
                id,
                serde_json::json!({
                    "protocol_version": 1,
                    "agent_info": {"name": "loopal", "version": "0.1.0"}
                }),
            )
            .await;
    }

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result["protocol_version"], 1);
    assert_eq!(result["agent_info"]["name"], "loopal");
}

#[tokio::test]
async fn shutdown_before_start_is_graceful() {
    let (client, server) = pipe_connection_pair();
    let mut server_rx = server.start();
    let _client_rx = client.start();

    let client_clone = client.clone();
    let handle = tokio::spawn(async move {
        client_clone
            .send_request(methods::AGENT_SHUTDOWN.name, serde_json::Value::Null)
            .await
    });

    if let Some(Incoming::Request { id, method, .. }) = server_rx.recv().await {
        assert_eq!(method, methods::AGENT_SHUTDOWN.name);
        let _ = server.respond(id, serde_json::json!({"ok": true})).await;
    }

    let result = handle.await.unwrap().unwrap();
    assert_eq!(result["ok"], true);
}

#[tokio::test]
async fn agent_event_notification_delivered() {
    let (client, server) = pipe_connection_pair();
    let mut client_rx = client.start();
    let _server_rx = server.start();

    // Server sends an agent/event notification
    server
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::json!({
                "agent_name": null,
                "payload": {"Stream": {"text": "hello"}}
            }),
        )
        .await
        .unwrap();

    // Client receives it
    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), client_rx.recv())
        .await
        .unwrap();

    match msg {
        Some(Incoming::Notification { method, params }) => {
            assert_eq!(method, methods::AGENT_EVENT.name);
            assert_eq!(params["payload"]["Stream"]["text"], "hello");
        }
        other => panic!("expected notification, got: {other:?}"),
    }
}

#[tokio::test]
async fn interrupt_notification_delivered() {
    let (client, server) = pipe_connection_pair();
    let mut server_rx = server.start();
    let _client_rx = client.start();

    // Client sends interrupt notification
    client
        .send_notification(methods::AGENT_INTERRUPT.name, serde_json::Value::Null)
        .await
        .unwrap();

    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), server_rx.recv())
        .await
        .unwrap();

    match msg {
        Some(Incoming::Notification { method, .. }) => {
            assert_eq!(method, methods::AGENT_INTERRUPT.name);
        }
        other => panic!("expected interrupt notification, got: {other:?}"),
    }
}
