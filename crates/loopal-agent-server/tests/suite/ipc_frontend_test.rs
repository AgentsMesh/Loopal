//! IpcFrontend unit tests — verifies emit and interrupt signal handling.

use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::StdioTransport;
use loopal_protocol::{AgentEventPayload, InterruptSignal};

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
    let interrupt = InterruptSignal::new();
    let frontend = loopal_agent_server::ipc_frontend_for_test(server_conn, server_rx, interrupt);

    frontend.emit(AgentEventPayload::AwaitingInput).await.unwrap();

    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), client_rx.recv())
        .await.unwrap().unwrap();
    match msg {
        Incoming::Notification { method, params } => {
            assert_eq!(method, methods::AGENT_EVENT.name);
            let event: loopal_protocol::AgentEvent = serde_json::from_value(params).unwrap();
            assert!(matches!(event.payload, AgentEventPayload::AwaitingInput));
        }
        _ => panic!("expected notification"),
    }
}

#[tokio::test]
async fn recv_input_sets_interrupt_on_notification() {
    #[allow(unused_imports)]
    use loopal_runtime::AgentFrontend;

    let (server_conn, server_rx, client_conn, _client_rx) = ipc_pair();
    let interrupt = InterruptSignal::new();
    let interrupt_check = interrupt.clone();
    let frontend = loopal_agent_server::ipc_frontend_for_test(
        server_conn, server_rx, interrupt,
    );

    // Send interrupt notification first
    client_conn
        .send_notification(methods::AGENT_INTERRUPT.name, serde_json::Value::Null)
        .await.unwrap();

    // Then send a message (as request) — recv_input will process interrupt, then message.
    // Must run client request and frontend recv_input in parallel.
    let client_clone = client_conn.clone();
    tokio::spawn(async move {
        let _ = client_clone.send_request(
            methods::AGENT_MESSAGE.name,
            serde_json::json!({
                "id": "00000000-0000-0000-0000-000000000000",
                "source": "Human", "target": "main",
                "content": {"text": "test", "images": []},
                "timestamp": "2024-01-01T00:00:00Z"
            }),
        ).await;
    });

    // recv_input processes interrupt notification (sets signal), then returns message
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), frontend.recv_input())
        .await;

    assert!(result.is_ok(), "recv_input should not timeout");
    assert!(interrupt_check.is_signaled(), "interrupt should be signaled");
}
