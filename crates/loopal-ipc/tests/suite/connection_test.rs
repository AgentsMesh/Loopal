use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::StdioTransport;

/// Create a pair of Connections backed by in-memory duplex streams.
fn connection_pair() -> (Arc<Connection>, Arc<Connection>) {
    let (a_tx, a_rx) = tokio::io::duplex(4096);
    let (b_tx, b_rx) = tokio::io::duplex(4096);

    let transport_a: Arc<dyn loopal_ipc::transport::Transport> =
        Arc::new(StdioTransport::new(
            Box::new(tokio::io::BufReader::new(b_rx)),
            Box::new(a_tx),
        ));
    let transport_b: Arc<dyn loopal_ipc::transport::Transport> =
        Arc::new(StdioTransport::new(
            Box::new(tokio::io::BufReader::new(a_rx)),
            Box::new(b_tx),
        ));

    (
        Arc::new(Connection::new(transport_a)),
        Arc::new(Connection::new(transport_b)),
    )
}

#[tokio::test]
async fn request_response_roundtrip() {
    let (client, server) = connection_pair();

    let mut server_rx = server.start();
    let _client_rx = client.start();

    // Client sends request, server responds
    let server_clone = server.clone();
    let handle = tokio::spawn(async move {
        if let Some(Incoming::Request { id, method, params }) = server_rx.recv().await {
            assert_eq!(method, "test/echo");
            server_clone
                .respond(id, params)
                .await
                .expect("respond ok");
        }
    });

    let result = client
        .send_request("test/echo", serde_json::json!({"msg": "hi"}))
        .await
        .expect("request ok");

    assert_eq!(result["msg"], "hi");
    handle.await.unwrap();
}

#[tokio::test]
async fn notification_delivery() {
    let (sender, receiver) = connection_pair();
    let mut rx = receiver.start();
    let _sender_rx = sender.start();

    sender
        .send_notification("event/update", serde_json::json!({"n": 42}))
        .await
        .expect("notify ok");

    match rx.recv().await.expect("should receive") {
        Incoming::Notification { method, params } => {
            assert_eq!(method, "event/update");
            assert_eq!(params["n"], 42);
        }
        _ => panic!("expected Notification"),
    }
}

#[tokio::test]
async fn error_response() {
    let (client, server) = connection_pair();
    let mut server_rx = server.start();
    let _client_rx = client.start();

    let server_clone = server.clone();
    tokio::spawn(async move {
        if let Some(Incoming::Request { id, .. }) = server_rx.recv().await {
            server_clone
                .respond_error(id, -32601, "not found")
                .await
                .expect("respond_error ok");
        }
    });

    let result = client
        .send_request("unknown", serde_json::json!(null))
        .await
        .expect("should get response");

    // Error responses are routed as-is to the pending channel
    assert_eq!(result["code"], -32601);
    assert_eq!(result["message"], "not found");
}
