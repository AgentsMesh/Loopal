//! Tests that the Connection reader task exits when the incoming channel is dropped.
//!
//! Verifies that after the consumer drops the receiver from `Connection::start()`,
//! the background reader task breaks on the next `tx.send()` failure rather than
//! silently continuing to read from the transport.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::Connection;

/// When the incoming receiver is dropped, the reader task should exit
/// after the next message arrives (tx.send fails → break).
///
/// Verification: after the reader exits, closing the remote writer causes
/// EOF on the read side. Since the reader is no longer holding the Mutex,
/// we can directly call recv() and observe the EOF immediately — proving
/// the reader is no longer blocking the transport.
#[tokio::test]
async fn reader_exits_after_incoming_channel_dropped() {
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let server_transport_ref = server_transport.clone();

    let client = Arc::new(Connection::new(client_transport));
    let server = Arc::new(Connection::new(server_transport));

    let server_rx = server.start();
    let _client_rx = client.start();

    // Drop the server's incoming receiver
    drop(server_rx);

    // Send a notification from client → server's reader will try tx.send() → fail → break
    let _ = client
        .send_notification("trigger", serde_json::json!({}))
        .await;

    // Give the reader task time to exit and release the read Mutex
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Close client's writer → EOF on server's read side.
    client.close().await;

    // If the reader task were still running, it would be holding the read Mutex
    // and we couldn't call recv(). Since the reader exited, we can recv() and
    // should see EOF immediately.
    let result = tokio::time::timeout(Duration::from_secs(1), server_transport_ref.recv()).await;

    match result {
        Ok(Ok(None)) => {} // EOF — reader is gone, we got the Mutex and saw EOF
        Ok(Ok(Some(_))) => panic!("should not receive data after remote closed"),
        Ok(Err(_)) => {} // read error also acceptable
        Err(_) => panic!("recv timed out — reader task likely still holding the Mutex"),
    }
}
