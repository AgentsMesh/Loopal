//! IpcFrontend unit tests — verifies emit and recv_input behavior.

use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEventPayload;

fn ipc_pair() -> (
    Arc<Connection>,
    tokio::sync::mpsc::Receiver<Incoming>,
    Arc<Connection>,
    tokio::sync::mpsc::Receiver<Incoming>,
) {
    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);
    let ta: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let tb: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    let ca = Arc::new(Connection::new(ta));
    let cb = Arc::new(Connection::new(tb));
    let ra = ca.start();
    let rb = cb.start();
    (ca, ra, cb, rb)
}

#[tokio::test]
async fn emit_sends_agent_event_notification() {
    #[allow(unused_imports)]
    use loopal_runtime::AgentFrontend;

    let (server_conn, server_rx, _client_conn, mut client_rx) = ipc_pair();
    let frontend = loopal_agent_server::ipc_frontend_for_test(server_conn, server_rx);

    frontend
        .emit(AgentEventPayload::AwaitingInput)
        .await
        .unwrap();

    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), client_rx.recv())
        .await
        .unwrap()
        .unwrap();
    match msg {
        Incoming::Notification { method, params } => {
            assert_eq!(method, methods::AGENT_EVENT.name);
            let event: loopal_protocol::AgentEvent = serde_json::from_value(params).unwrap();
            assert!(matches!(event.payload, AgentEventPayload::AwaitingInput));
        }
        _ => panic!("expected notification"),
    }
}

/// After interrupt_filter is wired in, recv_input no longer sees interrupt
/// notifications. This test verifies that recv_input skips non-interrupt
/// notifications and still returns the next request correctly.
#[tokio::test]
async fn recv_input_skips_unknown_notifications() {
    #[allow(unused_imports)]
    use loopal_runtime::AgentFrontend;

    let (server_conn, server_rx, client_conn, _client_rx) = ipc_pair();
    let frontend = loopal_agent_server::ipc_frontend_for_test(server_conn, server_rx);

    // Send an unknown notification first (simulating a non-interrupt notification)
    client_conn
        .send_notification("unknown/method", serde_json::Value::Null)
        .await
        .unwrap();

    // Then send a valid message request
    let client_clone = client_conn.clone();
    tokio::spawn(async move {
        let _ = client_clone
            .send_request(
                methods::AGENT_MESSAGE.name,
                serde_json::json!({
                    "id": "00000000-0000-0000-0000-000000000000",
                    "source": "Human", "target": "main",
                    "content": {"text": "test", "images": []},
                    "timestamp": "2024-01-01T00:00:00Z"
                }),
            )
            .await;
    });

    // recv_input should skip the notification and return the message
    let result =
        tokio::time::timeout(std::time::Duration::from_secs(2), frontend.recv_input()).await;
    assert!(result.is_ok(), "recv_input should not timeout");
    assert!(
        result.unwrap().is_some(),
        "recv_input should return a message"
    );
}
