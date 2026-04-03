//! Tests for DuplexTransport close() behavior — especially that each side
//! has an independent connected flag and can close independently.

use loopal_ipc::duplex_pair;

#[tokio::test]
async fn duplex_close_causes_remote_eof() {
    let (a, b) = duplex_pair();

    a.send(b"hello").await.unwrap();
    let msg = b.recv().await.unwrap().expect("should receive");
    assert_eq!(msg, b"hello");

    // Close A's writer → B's reader sees EOF
    a.close().await;
    let result = b.recv().await.unwrap();
    assert!(result.is_none(), "expected EOF after close");
}

#[tokio::test]
async fn duplex_close_one_side_then_other() {
    let (a, b) = duplex_pair();

    // Close A's writer
    a.close().await;
    assert!(!a.is_connected());

    // B should still be able to close its own writer independently
    assert!(b.is_connected());
    b.close().await;
    assert!(!b.is_connected());
}

#[tokio::test]
async fn duplex_both_sides_eof_after_mutual_close() {
    let (a, b) = duplex_pair();

    // Close both sides
    a.close().await;
    b.close().await;

    // Both readers should see EOF
    let ra = a.recv().await.unwrap();
    let rb = b.recv().await.unwrap();
    assert!(ra.is_none(), "A should see EOF");
    assert!(rb.is_none(), "B should see EOF");
}

#[tokio::test]
async fn duplex_send_after_close_fails() {
    let (a, _b) = duplex_pair();
    a.close().await;

    let result = a.send(b"nope").await;
    assert!(result.is_err(), "send after close should fail");
}
