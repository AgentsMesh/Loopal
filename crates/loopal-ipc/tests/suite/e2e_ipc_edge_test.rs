//! Edge-case e2e IPC tests: streaming order, interrupt, question, bridge close.

use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, AgentEventPayload};

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
    let sc = Arc::new(Connection::new(server_t));
    let sr = sc.start();
    (client_t, sc, sr)
}

#[tokio::test]
async fn e2e_streaming_events_ordered() {
    let (client_t, server_conn, _server_rx) = ipc_pair();
    let client = loopal_agent_client::AgentClient::new(client_t);
    let (conn, incoming) = client.into_parts();
    let handles = loopal_agent_client::start_bridge(conn, incoming);

    for i in 0..5 {
        let event = AgentEvent {
            agent_name: None,
            event_id: 0,
            turn_id: 0,
            correlation_id: 0,
            rev: None,
            payload: AgentEventPayload::Stream {
                text: format!("chunk-{i}"),
            },
        };
        server_conn
            .send_notification(
                methods::AGENT_EVENT.name,
                serde_json::to_value(&event).unwrap(),
            )
            .await
            .unwrap();
    }

    let mut rx = handles.agent_event_rx;
    let mut texts = Vec::new();
    for _ in 0..5 {
        let ev = tokio::time::timeout(TIMEOUT, rx.recv())
            .await
            .unwrap()
            .unwrap();
        if let AgentEventPayload::Stream { text } = ev.payload {
            texts.push(text);
        }
    }
    assert_eq!(
        texts,
        vec!["chunk-0", "chunk-1", "chunk-2", "chunk-3", "chunk-4"]
    );
}

#[tokio::test]
async fn e2e_interrupt_notification() {
    let (client_t, _server_conn, mut server_rx) = ipc_pair();
    let client = loopal_agent_client::AgentClient::new(client_t);
    let (conn, _incoming) = client.into_parts();

    conn.send_notification(methods::AGENT_INTERRUPT.name, serde_json::Value::Null)
        .await
        .unwrap();

    let msg = tokio::time::timeout(TIMEOUT, server_rx.recv())
        .await
        .unwrap()
        .unwrap();
    match msg {
        Incoming::Notification { method, .. } => assert_eq!(method, methods::AGENT_INTERRUPT.name),
        _ => panic!("expected interrupt notification"),
    }
}

#[tokio::test]
async fn e2e_bridge_stops_on_incoming_close() {
    let (client_t, _server_conn, _server_rx) = ipc_pair();
    let client = loopal_agent_client::AgentClient::new(client_t);
    let (conn, _incoming) = client.into_parts();

    let (fwd_tx, fwd_rx) = tokio::sync::mpsc::channel(16);
    let handles = loopal_agent_client::start_bridge(conn, fwd_rx);

    let event = AgentEvent {
        agent_name: None,
        event_id: 0,
        turn_id: 0,
        correlation_id: 0,
        rev: None,
        payload: AgentEventPayload::Stream { text: "one".into() },
    };
    fwd_tx
        .send(Incoming::Notification {
            method: methods::AGENT_EVENT.name.into(),
            params: serde_json::to_value(&event).unwrap(),
        })
        .await
        .unwrap();

    let mut rx = handles.agent_event_rx;
    drop(handles.agent_event_tx); // Drop clone so bridge shutdown closes channel
    let ev = tokio::time::timeout(TIMEOUT, rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(ev.payload, AgentEventPayload::Stream { .. }));

    drop(fwd_tx);
    let result = tokio::time::timeout(TIMEOUT, rx.recv()).await.unwrap();
    assert!(result.is_none());
}
