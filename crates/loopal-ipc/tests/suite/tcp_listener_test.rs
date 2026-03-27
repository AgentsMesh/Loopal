use std::sync::Arc;

use loopal_ipc::transport::Transport;
use loopal_ipc::{Connection, IpcListener, TcpTransport};

/// IpcListener generates a token and binds to a random port.
#[tokio::test]
async fn listener_port_and_token() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    assert!(listener.port() > 0);
    assert!(!listener.token().is_empty());
}

/// Two listeners get different tokens.
#[tokio::test]
async fn listener_unique_tokens() {
    let a = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let b = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    assert_ne!(a.token(), b.token());
}

/// Full JSON-RPC roundtrip over TCP via Connection.
#[tokio::test]
async fn tcp_connection_jsonrpc_roundtrip() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    let server_task = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let conn = Arc::new(Connection::new(Arc::new(transport)));
        let mut rx = conn.start();

        // Receive request from client
        let incoming = rx.recv().await.expect("should receive");
        match incoming {
            loopal_ipc::connection::Incoming::Request { id, method, .. } => {
                assert_eq!(method, "test/hello");
                conn.respond(id, serde_json::json!({"reply": "world"}))
                    .await
                    .unwrap();
            }
            _ => panic!("expected request"),
        }
    });

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let client_conn = Arc::new(Connection::new(Arc::new(TcpTransport::new(stream))));
    let _rx = client_conn.start();

    let result = client_conn
        .send_request("test/hello", serde_json::json!({"name": "loopal"}))
        .await
        .unwrap();
    assert_eq!(result["reply"], "world");

    server_task.await.unwrap();
}

/// Multiple clients can connect to the same listener sequentially.
#[tokio::test]
async fn listener_accepts_multiple_clients() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    // Accept two clients
    let server = tokio::spawn(async move {
        for expected in ["client_a", "client_b"] {
            let (transport, _) = listener.accept().await.unwrap();
            let msg = transport.recv().await.unwrap().expect("msg");
            assert_eq!(String::from_utf8(msg).unwrap(), expected);
        }
    });

    for name in ["client_a", "client_b"] {
        let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        let t = TcpTransport::new(stream);
        t.send(name.as_bytes()).await.unwrap();
    }

    server.await.unwrap();
}
