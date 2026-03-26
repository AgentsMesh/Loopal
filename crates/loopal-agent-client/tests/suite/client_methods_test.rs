//! Additional AgentClient method tests (send_control, send_interrupt, send_message).

use std::sync::Arc;

use loopal_ipc::StdioTransport;
use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;

use loopal_agent_client::AgentClient;

fn make_pair() -> (Arc<dyn loopal_ipc::transport::Transport>, Arc<Connection>) {
    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);
    let ct: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(b_rx)),
        Box::new(a_tx),
    ));
    let st: Arc<dyn loopal_ipc::transport::Transport> = Arc::new(StdioTransport::new(
        Box::new(tokio::io::BufReader::new(a_rx)),
        Box::new(b_tx),
    ));
    (ct, Arc::new(Connection::new(st)))
}

#[tokio::test]
async fn send_control_delivers_to_server() {
    let (transport, server) = make_pair();
    let mut server_rx = server.start();
    let client = AgentClient::new(transport);

    let sc = server.clone();
    tokio::spawn(async move {
        if let Some(Incoming::Request { id, .. }) = server_rx.recv().await {
            let _ = sc.respond(id, serde_json::json!({"ok": true})).await;
        }
    });

    client
        .send_control(&loopal_protocol::ControlCommand::Compact)
        .await
        .unwrap();
}

#[tokio::test]
async fn send_interrupt_delivers_notification() {
    let (transport, server) = make_pair();
    let mut server_rx = server.start();
    let client = AgentClient::new(transport);

    client.send_interrupt().await.unwrap();

    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), server_rx.recv())
        .await
        .unwrap()
        .unwrap();
    match msg {
        Incoming::Notification { method, .. } => assert_eq!(method, methods::AGENT_INTERRUPT.name),
        _ => panic!("expected notification"),
    }
}

#[tokio::test]
async fn send_message_delivers_envelope() {
    let (transport, server) = make_pair();
    let mut server_rx = server.start();
    let client = AgentClient::new(transport);

    let sc = server.clone();
    tokio::spawn(async move {
        if let Some(Incoming::Request { id, .. }) = server_rx.recv().await {
            let _ = sc.respond(id, serde_json::json!({"ok": true})).await;
        }
    });

    let envelope = loopal_protocol::Envelope {
        id: uuid::Uuid::new_v4(),
        source: loopal_protocol::MessageSource::Human,
        target: "main".into(),
        content: loopal_protocol::UserContent::text_only("test"),
        timestamp: chrono::Utc::now(),
    };
    client.send_message(&envelope).await.unwrap();
}
