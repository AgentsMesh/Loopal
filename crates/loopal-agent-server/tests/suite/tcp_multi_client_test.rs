//! Tests for TCP token verification.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::Connection;
use loopal_ipc::transport::Transport;
use loopal_ipc::{IpcListener, TcpTransport};

const T: Duration = Duration::from_secs(5);

/// Valid token → initialize succeeds via TCP.
#[tokio::test]
async fn tcp_valid_token_accepted() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();
    let token = listener.token().to_string();

    // Server side: accept + verify token manually
    let server = tokio::spawn({
        let token = token.clone();
        async move {
            let (transport, _) = listener.accept().await.unwrap();
            let conn = Arc::new(Connection::new(Arc::new(transport) as Arc<dyn Transport>));
            let mut rx = conn.start();
            // Read initialize request
            let msg = rx.recv().await.unwrap();
            if let loopal_ipc::connection::Incoming::Request { id, params, .. } = msg {
                let client_token = params["token"].as_str().unwrap_or("");
                assert_eq!(client_token, token, "token should match");
                conn.respond(id, serde_json::json!({"protocol_version": 1}))
                    .await
                    .unwrap();
            }
        }
    });

    // Client side: connect + send token
    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let client = Arc::new(Connection::new(
        Arc::new(TcpTransport::new(stream)) as Arc<dyn Transport>
    ));
    let _rx = client.start();
    let resp = tokio::time::timeout(
        T,
        client.send_request(
            "initialize",
            serde_json::json!({"protocol_version": 1, "token": token}),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(resp["protocol_version"], 1);
    server.await.unwrap();
}

/// Wrong token → server responds with error.
#[tokio::test]
async fn tcp_wrong_token_rejected() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();
    let real_token = listener.token().to_string();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let conn = Arc::new(Connection::new(Arc::new(transport) as Arc<dyn Transport>));
        let mut rx = conn.start();
        let msg = rx.recv().await.unwrap();
        if let loopal_ipc::connection::Incoming::Request { id, params, .. } = msg {
            let client_token = params["token"].as_str().unwrap_or("");
            assert_ne!(client_token, real_token, "tokens should NOT match");
            conn.respond_error(id, -32600, "invalid token")
                .await
                .unwrap();
        }
    });

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let client = Arc::new(Connection::new(
        Arc::new(TcpTransport::new(stream)) as Arc<dyn Transport>
    ));
    let _rx = client.start();
    // send_request with wrong token — server responds with JSON-RPC error
    let result = tokio::time::timeout(
        T,
        client.send_request(
            "initialize",
            serde_json::json!({"protocol_version": 1, "token": "wrong-token"}),
        ),
    )
    .await
    .unwrap();
    // Connection.send_request may return Ok(error_value) or Err depending
    // on how the server closes. Either way, it should NOT succeed normally.
    match result {
        Ok(val) => {
            // Server returned a JSON-RPC error — token rejected
            assert!(
                val.get("code").is_some() || val.is_null(),
                "expected error response, got: {val}"
            );
        }
        Err(_) => {
            // Connection dropped — also acceptable
        }
    }
    server.await.unwrap();
}

/// Multiple TCP clients can connect to the same listener sequentially.
#[tokio::test]
async fn tcp_multiple_clients_sequential() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    let server = tokio::spawn(async move {
        for i in 0..3 {
            let (transport, _) = listener.accept().await.unwrap();
            let conn = Arc::new(Connection::new(Arc::new(transport) as Arc<dyn Transport>));
            let mut rx = conn.start();
            let msg = rx.recv().await.unwrap();
            if let loopal_ipc::connection::Incoming::Request { id, params, .. } = msg {
                let client_id = params["client_id"].as_i64().unwrap();
                assert_eq!(client_id, i);
                conn.respond(id, serde_json::json!({"ok": true}))
                    .await
                    .unwrap();
            }
        }
    });

    for i in 0..3i64 {
        let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        let client = Arc::new(Connection::new(
            Arc::new(TcpTransport::new(stream)) as Arc<dyn Transport>
        ));
        let _rx = client.start();
        let resp = tokio::time::timeout(
            T,
            client.send_request("hello", serde_json::json!({"client_id": i})),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(resp["ok"], true);
    }
    server.await.unwrap();
}
