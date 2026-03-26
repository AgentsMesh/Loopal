//! Edge case tests for Connection — EOF, pending cleanup, concurrent requests.

use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};

fn connection_pair() -> (Arc<Connection>, Arc<Connection>) {
    let (a_tx, a_rx) = tokio::io::duplex(4096);
    let (b_tx, b_rx) = tokio::io::duplex(4096);
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
async fn pending_requests_cleared_on_eof() {
    let (a_tx, a_rx) = tokio::io::duplex(4096);
    let (b_tx, b_rx) = tokio::io::duplex(4096);

    let ta: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let client = Arc::new(Connection::new(ta));
    let _client_rx = client.start();

    // Drop the other end to cause EOF
    drop(b_tx);
    drop(a_rx);

    // send_request should fail because reader loop exits and drops pending sender
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client.send_request("test", serde_json::json!(null)),
    )
    .await;

    match result {
        Ok(Err(msg)) => assert!(msg.contains("dropped") || msg.contains("failed")),
        Err(_) => panic!("should not timeout — pending should be cleaned up"),
        Ok(Ok(_)) => panic!("should not succeed with closed transport"),
    }
}

#[tokio::test]
async fn concurrent_requests_each_get_correct_response() {
    let (client, server) = connection_pair();
    let mut server_rx = server.start();
    let _client_rx = client.start();

    // Server echoes back with the request ID embedded
    let server_clone = server.clone();
    tokio::spawn(async move {
        while let Some(Incoming::Request { id, params, .. }) = server_rx.recv().await {
            let _ = server_clone
                .respond(id, serde_json::json!({"echo": params}))
                .await;
        }
    });

    // Fire 10 concurrent requests
    let mut handles = Vec::new();
    for i in 0..10 {
        let c = client.clone();
        handles.push(tokio::spawn(async move {
            let result = c
                .send_request("echo", serde_json::json!({"n": i}))
                .await
                .unwrap();
            result["echo"]["n"].as_i64().unwrap()
        }));
    }

    let mut results: Vec<i64> = Vec::new();
    for h in handles {
        results.push(h.await.unwrap());
    }
    results.sort();
    assert_eq!(results, (0..10).collect::<Vec<_>>());
}

#[tokio::test]
async fn incoming_channel_returns_none_on_eof() {
    let (a_tx, _a_rx) = tokio::io::duplex(4096);
    let (_b_tx, b_rx) = tokio::io::duplex(4096);

    let t: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let conn = Arc::new(Connection::new(t));
    let mut rx = conn.start();

    // Drop sender side → EOF
    drop(_b_tx);

    // incoming channel should return None
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await;
    match result {
        Ok(None) => {} // correct
        Ok(Some(_)) => panic!("should not receive messages after EOF"),
        Err(_) => panic!("should not timeout"),
    }
}
