//! Edge case tests for Connection — EOF, pending cleanup, concurrent requests.

use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming, Listening};
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
    let ta: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let tb: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    let (client, client_rx) = Connection::new(ta).into_listening();
    let (server, server_rx) = Connection::new(tb).into_listening();
    ConnPair {
        client,
        client_rx,
        server,
        server_rx,
    }
}

#[tokio::test]
async fn pending_requests_cleared_on_eof() {
    let (a_tx, a_rx) = tokio::io::duplex(4096);
    let (b_tx, b_rx) = tokio::io::duplex(4096);

    let ta: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let (client, _client_rx) = Connection::new(ta).into_listening();

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
        Ok(Err(loopal_ipc::RpcError::ChannelDropped)) => {}
        Ok(Err(loopal_ipc::RpcError::Transport(_))) => {}
        Ok(other) => panic!("expected ChannelDropped or Transport, got: {other:?}"),
        Err(_) => panic!("should not timeout — pending should be cleaned up"),
    }
}

#[tokio::test]
async fn concurrent_requests_each_get_correct_response() {
    let ConnPair {
        client,
        server,
        mut server_rx,
        ..
    } = connection_pair();

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
async fn dropping_request_notifies_peer_of_cancellation() {
    let ConnPair {
        client,
        mut server_rx,
        ..
    } = connection_pair();

    let request = tokio::spawn(async move {
        client
            .send_request("agent/plan_approval", serde_json::json!({}))
            .await
    });
    let request_id = match server_rx.recv().await.unwrap() {
        Incoming::Request { id, method, .. } => {
            assert_eq!(method, "agent/plan_approval");
            id
        }
        other => panic!("expected request, got {other:?}"),
    };

    request.abort();
    let _ = request.await;

    let cancelled = tokio::time::timeout(std::time::Duration::from_secs(2), server_rx.recv())
        .await
        .expect("peer must receive cancellation")
        .expect("peer connection must remain open");
    match cancelled {
        Incoming::Notification { method, params } => {
            assert_eq!(method, "$/cancelRequest");
            assert_eq!(params["id"], request_id);
            assert_eq!(params["method"], "agent/plan_approval");
        }
        other => panic!("expected cancellation notification, got {other:?}"),
    }
}

#[tokio::test]
async fn incoming_channel_returns_none_on_eof() {
    let (a_tx, _a_rx) = tokio::io::duplex(4096);
    let (_b_tx, b_rx) = tokio::io::duplex(4096);

    let t: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let (_conn, mut rx) = Connection::new(t).into_listening();

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
