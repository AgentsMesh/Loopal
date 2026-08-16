use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::{Connection, Incoming, Listening};

use super::core_tests::{notification, peers, request_parts, session};
use super::forward_with_timeout;

async fn request_raw_parts(
    client: Arc<Connection<Listening>>,
    incoming: &mut tokio::sync::mpsc::Receiver<Incoming>,
    params: serde_json::Value,
) -> (
    tokio::task::JoinHandle<Result<serde_json::Value, loopal_ipc::RpcError>>,
    i64,
    serde_json::Value,
) {
    let request = tokio::spawn(async move { client.send_request("terminal-test", params).await });
    let Incoming::Request { id, params, .. } = incoming.recv().await.unwrap() else {
        panic!("expected request")
    };
    (request, id, params)
}

#[tokio::test]
async fn malformed_and_invalid_notifications_fail_closed_before_enqueue() {
    let ((server, mut incoming), (client, _)) = peers();
    let (session, mut input_rx) = session("session-validation");

    let (malformed, id, params) = request_raw_parts(
        client.clone(),
        &mut incoming,
        serde_json::json!({"unexpected": true}),
    )
    .await;
    forward_with_timeout(id, params, &session, &server, Duration::ZERO).await;
    assert_eq!(
        malformed.await.unwrap().unwrap_err().remote_code(),
        Some(loopal_ipc::jsonrpc::INVALID_REQUEST)
    );

    let mut invalid = notification("session-validation");
    invalid.delivery_id.terminal_revision = 0;
    let (invalid_response, id, params) = request_parts(client, &mut incoming, &invalid).await;
    forward_with_timeout(id, params, &session, &server, Duration::ZERO).await;
    assert_eq!(
        invalid_response.await.unwrap().unwrap_err().remote_code(),
        Some(loopal_ipc::jsonrpc::INVALID_REQUEST)
    );
    assert!(input_rx.try_recv().is_err());
}
