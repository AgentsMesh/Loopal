//! Bridge integration tests — verifies IPC-to-channel forwarding.

use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;

fn make_pair() -> (Arc<Connection>, Arc<Connection>) {
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
    (Arc::new(Connection::new(ta)), Arc::new(Connection::new(tb)))
}

#[tokio::test]
async fn bridge_forwards_agent_events_to_channel() {
    let (agent_conn, tui_conn) = make_pair();
    let tui_incoming = tui_conn.start();
    let _agent_rx = agent_conn.start();

    // Start bridge with the TUI-side connection
    let handles = loopal_agent_client::start_bridge(tui_conn, tui_incoming);

    // Agent sends event notification
    agent_conn
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::json!({
                "agent_name": null,
                "payload": {"AwaitingInput": null}
            }),
        )
        .await
        .unwrap();

    // TUI channel receives it
    let mut rx = handles.agent_event_rx;
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .unwrap();

    match event {
        Some(ev) => match ev.payload {
            loopal_protocol::AgentEventPayload::AwaitingInput => {}
            other => panic!("expected AwaitingInput, got: {other:?}"),
        },
        None => panic!("expected event, got None"),
    }
}

#[tokio::test]
async fn bridge_forwards_control_commands_to_ipc() {
    let (agent_conn, tui_conn) = make_pair();
    let tui_incoming = tui_conn.start();
    let mut agent_rx = agent_conn.start();

    let handles = loopal_agent_client::start_bridge(tui_conn, tui_incoming);

    // TUI sends control command through bridge
    handles
        .control_tx
        .send(loopal_protocol::ControlCommand::Clear)
        .await
        .unwrap();

    // Agent receives it as a request
    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), agent_rx.recv())
        .await
        .unwrap();

    match msg {
        Some(loopal_ipc::connection::Incoming::Request { method, .. }) => {
            assert_eq!(method, methods::AGENT_CONTROL.name);
        }
        other => panic!("expected control request, got: {other:?}"),
    }
}

#[tokio::test]
async fn bridge_forwards_mailbox_messages_to_ipc() {
    let (agent_conn, tui_conn) = make_pair();
    let tui_incoming = tui_conn.start();
    let mut agent_rx = agent_conn.start();

    let handles = loopal_agent_client::start_bridge(tui_conn, tui_incoming);

    // Build an Envelope
    let envelope = loopal_protocol::Envelope {
        id: uuid::Uuid::new_v4(),
        source: loopal_protocol::MessageSource::Human,
        target: "main".into(),
        content: loopal_protocol::UserContent::text_only("hello"),
        timestamp: chrono::Utc::now(),
    };

    handles.mailbox_tx.send(envelope).await.unwrap();

    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), agent_rx.recv())
        .await
        .unwrap();

    match msg {
        Some(loopal_ipc::connection::Incoming::Request { method, params, .. }) => {
            assert_eq!(method, methods::AGENT_MESSAGE.name);
            // QualifiedAddress serializes as { "hub": [...], "agent": "name" }
            assert_eq!(params["target"]["agent"], "main");
            assert!(params["target"]["hub"].as_array().unwrap().is_empty());
        }
        other => panic!("expected message request, got: {other:?}"),
    }
}
