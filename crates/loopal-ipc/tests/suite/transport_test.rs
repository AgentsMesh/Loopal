use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::transport::Transport;

/// Create a pair of connected StdioTransports using in-memory pipes.
fn pipe_pair() -> (Arc<StdioTransport>, Arc<StdioTransport>) {
    let (a_tx, a_rx) = tokio::io::duplex(4096);
    let (b_tx, b_rx) = tokio::io::duplex(4096);

    let a = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let b = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    (a, b)
}

#[tokio::test]
async fn send_recv_roundtrip() {
    let (a, b) = pipe_pair();

    a.send(b"hello world").await.unwrap();
    let received = b.recv().await.unwrap().expect("should receive");
    assert_eq!(received, b"hello world");
}

#[tokio::test]
async fn multiple_messages() {
    let (a, b) = pipe_pair();

    a.send(b"msg1").await.unwrap();
    a.send(b"msg2").await.unwrap();
    a.send(b"msg3").await.unwrap();

    let m1 = b.recv().await.unwrap().expect("msg1");
    let m2 = b.recv().await.unwrap().expect("msg2");
    let m3 = b.recv().await.unwrap().expect("msg3");

    assert_eq!(m1, b"msg1");
    assert_eq!(m2, b"msg2");
    assert_eq!(m3, b"msg3");
}

#[tokio::test]
async fn bidirectional() {
    let (a, b) = pipe_pair();

    a.send(b"from_a").await.unwrap();
    b.send(b"from_b").await.unwrap();

    let recv_at_b = b.recv().await.unwrap().expect("at b");
    let recv_at_a = a.recv().await.unwrap().expect("at a");

    assert_eq!(recv_at_b, b"from_a");
    assert_eq!(recv_at_a, b"from_b");
}

#[tokio::test]
async fn eof_returns_none() {
    let (a_tx, _a_rx) = tokio::io::duplex(4096);
    let (_b_tx, b_rx) = tokio::io::duplex(4096);

    let transport = StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    );

    // Drop the writer side to cause EOF
    drop(_b_tx);

    let result = transport.recv().await.unwrap();
    assert!(result.is_none());
    assert!(!transport.is_connected());
}

#[tokio::test]
async fn is_connected_initially_true() {
    let (a, b) = pipe_pair();
    assert!(a.is_connected());
    assert!(b.is_connected());
}
