use std::sync::Arc;

use loopal_ipc::transport::Transport;
use loopal_ipc::{IpcListener, TcpTransport};

#[tokio::test]
async fn tcp_send_recv_roundtrip() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    let server = tokio::spawn(async move {
        let (transport, _addr) = listener.accept().await.unwrap();
        let transport = Arc::new(transport);
        let msg = transport.recv().await.unwrap().expect("should receive");
        assert_eq!(msg, b"hello tcp");
        transport.send(b"hello back").await.unwrap();
    });

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let client = Arc::new(TcpTransport::new(stream));

    client.send(b"hello tcp").await.unwrap();
    let reply = client.recv().await.unwrap().expect("should receive reply");
    assert_eq!(reply, b"hello back");

    server.await.unwrap();
}

#[tokio::test]
async fn tcp_multiple_messages() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        for i in 0..3 {
            let msg = transport.recv().await.unwrap().expect("msg");
            assert_eq!(msg, format!("msg{i}").into_bytes());
        }
    });

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let client = TcpTransport::new(stream);
    for i in 0..3 {
        client.send(format!("msg{i}").as_bytes()).await.unwrap();
    }

    server.await.unwrap();
}

#[tokio::test]
async fn tcp_eof_returns_none() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        // Client drops → EOF
        let result = transport.recv().await.unwrap();
        assert!(result.is_none());
        assert!(!transport.is_connected());
    });

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    drop(stream); // Close immediately

    server.await.unwrap();
}

#[tokio::test]
async fn tcp_is_connected_initially_true() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        assert!(transport.is_connected());
    });

    let _stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();

    server.await.unwrap();
}

// ── close() tests ──────────────────────────────────────────────────

#[tokio::test]
async fn tcp_close_causes_remote_eof() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let transport = Arc::new(transport);
        // Confirm connection works
        let msg = transport.recv().await.unwrap().expect("should receive");
        assert_eq!(msg, b"ping");
        // Close writer side
        transport.close().await;
    });

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let client = Arc::new(TcpTransport::new(stream));
    client.send(b"ping").await.unwrap();

    // Wait for server to close, then recv should return EOF
    server.await.unwrap();
    let result = client.recv().await.unwrap();
    assert!(result.is_none(), "expected EOF after remote close");
}

#[tokio::test]
async fn tcp_send_after_close_fails() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    let server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        // Keep server alive while client tests
        let _ = transport.recv().await;
    });

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let client = TcpTransport::new(stream);
    client.close().await;

    let result = client.send(b"should_fail").await;
    assert!(result.is_err(), "send after close should fail");
    drop(server);
}

#[tokio::test]
async fn tcp_close_marks_disconnected() {
    let listener = IpcListener::bind("127.0.0.1:0".parse().unwrap())
        .await
        .unwrap();
    let port = listener.port();

    let _server = tokio::spawn(async move {
        let (transport, _) = listener.accept().await.unwrap();
        let _ = transport.recv().await;
    });

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .unwrap();
    let client = TcpTransport::new(stream);
    assert!(client.is_connected());

    client.close().await;
    assert!(!client.is_connected());
}
