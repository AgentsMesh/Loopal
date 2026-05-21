use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
use loopal_ipc::rpc_error::RpcError;
use tokio::sync::mpsc;

struct ConnPair {
    client: Arc<Connection<Listening>>,
    #[allow(dead_code)]
    client_rx: mpsc::Receiver<Incoming>,
    server: Arc<Connection<Listening>>,
    server_rx: mpsc::Receiver<Incoming>,
}

fn connection_pair() -> ConnPair {
    let (a_tx, a_rx) = tokio::io::duplex(4096);
    let (b_tx, b_rx) = tokio::io::duplex(4096);

    let transport_a: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let transport_b: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));

    let (client, client_rx) = Connection::new(transport_a).into_listening();
    let (server, server_rx) = Connection::new(transport_b).into_listening();
    ConnPair {
        client,
        client_rx,
        server,
        server_rx,
    }
}

#[tokio::test]
async fn request_response_roundtrip() {
    let ConnPair {
        client,
        server,
        mut server_rx,
        ..
    } = connection_pair();

    let server_clone = server.clone();
    let handle = tokio::spawn(async move {
        if let Some(Incoming::Request { id, method, params }) = server_rx.recv().await {
            assert_eq!(method, "test/echo");
            server_clone.respond(id, params).await.expect("respond ok");
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
    let ConnPair {
        client: sender,
        server_rx: mut rx,
        ..
    } = connection_pair();

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
    let ConnPair {
        client,
        server,
        mut server_rx,
        ..
    } = connection_pair();

    let server_clone = server.clone();
    tokio::spawn(async move {
        if let Some(Incoming::Request { id, .. }) = server_rx.recv().await {
            server_clone
                .respond_error(id, -32601, "not found")
                .await
                .expect("respond_error ok");
        }
    });

    let outcome = client
        .send_request("unknown", serde_json::json!(null))
        .await;

    let err = outcome.expect_err("rpc error should become Err");
    match err {
        RpcError::Remote { code, message, .. } => {
            assert_eq!(code, -32601);
            assert_eq!(message, "not found");
        }
        _ => panic!("expected Remote, got: {err:?}"),
    }
}
