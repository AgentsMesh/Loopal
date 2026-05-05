//! Regression tests for silent data loss scenarios found in round 9 review.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::AgentEventPayload;

const TIMEOUT: Duration = Duration::from_secs(5);

fn ipc_pair() -> (
    Arc<dyn loopal_ipc::transport::Transport>,
    Arc<Connection>,
    tokio::sync::mpsc::Receiver<Incoming>,
) {
    let (a_tx, a_rx) = tokio::io::duplex(16384);
    let (b_tx, b_rx) = tokio::io::duplex(16384);
    let client_t: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let server_t: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    let server_conn = Arc::new(Connection::new(server_t));
    let server_rx = server_conn.start();
    (client_t, server_conn, server_rx)
}

/// Malformed event notification should not crash bridge; valid events after it still arrive.
#[tokio::test]
async fn bridge_survives_malformed_event_notification() {
    let (client_t, server_conn, _server_rx) = ipc_pair();

    let client = loopal_agent_client::AgentClient::new(client_t);
    let (conn, incoming) = client.into_parts();
    let handles = loopal_agent_client::start_bridge(conn, incoming);

    // Send malformed event (invalid payload structure)
    server_conn
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::json!({"agent_name": null, "payload": "not_a_valid_enum"}),
        )
        .await
        .unwrap();

    // Send valid event after
    let valid = loopal_protocol::AgentEvent {
        agent_name: None,
        event_id: 0,
        turn_id: 0,
        correlation_id: 0,
        rev: None,
        payload: AgentEventPayload::AwaitingInput,
    };
    server_conn
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::to_value(&valid).unwrap(),
        )
        .await
        .unwrap();

    // Bridge should skip malformed and deliver valid
    let mut rx = handles.agent_event_rx;
    let ev = tokio::time::timeout(TIMEOUT, rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(ev.payload, AgentEventPayload::AwaitingInput));
}

/// Client.recv() skips malformed events and delivers the next valid one.
#[tokio::test]
async fn client_recv_survives_malformed_event() {
    let (client_t, server_conn, _server_rx) = ipc_pair();
    let mut client = loopal_agent_client::AgentClient::new(client_t);

    // Send malformed then valid
    server_conn
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::json!({"broken": true}),
        )
        .await
        .unwrap();

    let valid = loopal_protocol::AgentEvent {
        agent_name: Some("test".into()),
        event_id: 0,
        turn_id: 0,
        correlation_id: 0,
        rev: None,
        payload: AgentEventPayload::Finished,
    };
    server_conn
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::to_value(&valid).unwrap(),
        )
        .await
        .unwrap();

    let ev = tokio::time::timeout(TIMEOUT, client.recv())
        .await
        .unwrap()
        .unwrap();
    match ev {
        loopal_agent_client::AgentClientEvent::AgentEvent(e) => {
            assert!(matches!(e.payload, AgentEventPayload::Finished));
        }
    }
}
