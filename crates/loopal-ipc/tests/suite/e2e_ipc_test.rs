//! End-to-end IPC integration tests.
//!
//! Simulates the full multi-process IPC pipeline using in-memory duplex streams.
//! Tests the complete chain: client → IPC → server → IPC → bridge → TUI channels.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload, ControlCommand};

const TIMEOUT: Duration = Duration::from_secs(5);

/// Create a connected pair: (client_transport, server_connection + server_rx).
/// Client transport goes to AgentClient; server connection simulates IpcFrontend.
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

// ── Flow A: Message → Event round-trip ───────────────────────────────

#[tokio::test]
async fn e2e_message_then_event_roundtrip() {
    let (client_t, server_conn, mut server_rx) = ipc_pair();

    // Client handshake → bridge
    let client = loopal_agent_client::AgentClient::new(client_t);
    let (conn, incoming) = client.into_parts();
    let handles = loopal_agent_client::start_bridge(conn, incoming);

    // Server: respond to message, then emit event
    let sc = server_conn.clone();
    tokio::spawn(async move {
        if let Some(Incoming::Request { id, method, .. }) = server_rx.recv().await {
            assert_eq!(method, methods::AGENT_MESSAGE.name);
            let _ = sc.respond(id, serde_json::json!({"ok": true})).await;
            let event = AgentEvent {
                agent_name: None,
                event_id: 0,
                turn_id: 0,
                correlation_id: 0,
                rev: None,
                payload: AgentEventPayload::Stream {
                    text: "reply".into(),
                },
            };
            let _ = sc
                .send_notification(
                    methods::AGENT_EVENT.name,
                    serde_json::to_value(&event).unwrap(),
                )
                .await;
        }
    });

    // TUI sends message via bridge
    let envelope = loopal_protocol::Envelope {
        id: uuid::Uuid::new_v4(),
        source: loopal_protocol::MessageSource::Human,
        target: "main".into(),
        content: loopal_protocol::UserContent::text_only("hello"),
        timestamp: chrono::Utc::now(),
        summary: None,
    };
    handles.mailbox_tx.send(envelope).await.unwrap();

    // TUI receives event via bridge
    let mut rx = handles.agent_event_rx;
    let ev = tokio::time::timeout(TIMEOUT, rx.recv())
        .await
        .unwrap()
        .unwrap();
    match ev.payload {
        AgentEventPayload::Stream { text } => assert_eq!(text, "reply"),
        other => panic!("expected Stream, got: {other:?}"),
    }
}

// ── Flow C: Control command flows through bridge ─────────────────────

#[tokio::test]
async fn e2e_control_command_via_bridge() {
    let (client_t, _server_conn, mut server_rx) = ipc_pair();

    let client = loopal_agent_client::AgentClient::new(client_t);
    let (conn, incoming) = client.into_parts();
    let handles = loopal_agent_client::start_bridge(conn, incoming);

    handles
        .control_tx
        .send(ControlCommand::Clear)
        .await
        .unwrap();

    let msg = tokio::time::timeout(TIMEOUT, server_rx.recv())
        .await
        .unwrap()
        .unwrap();

    if let Incoming::Request { method, params, .. } = msg {
        assert_eq!(method, methods::AGENT_CONTROL.name);
        let cmd: ControlCommand = serde_json::from_value(params).unwrap();
        assert!(matches!(cmd, ControlCommand::Clear));
    } else {
        panic!("expected request");
    }
}
