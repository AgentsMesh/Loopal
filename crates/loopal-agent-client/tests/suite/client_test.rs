//! AgentClient protocol tests — handshake, recv events, permission responses.

use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming};
use loopal_ipc::protocol::methods;
use loopal_ipc::StdioTransport;

use loopal_agent_client::AgentClient;

fn make_pair() -> (Arc<dyn loopal_ipc::transport::Transport>, Arc<Connection>) {
    let (a_tx, a_rx) = tokio::io::duplex(8192);
    let (b_tx, b_rx) = tokio::io::duplex(8192);
    let client_transport: Arc<dyn loopal_ipc::transport::Transport> =
        Arc::new(StdioTransport::new(
            Box::new(tokio::io::BufReader::new(b_rx)),
            Box::new(a_tx),
        ));
    let server_transport: Arc<dyn loopal_ipc::transport::Transport> =
        Arc::new(StdioTransport::new(
            Box::new(tokio::io::BufReader::new(a_rx)),
            Box::new(b_tx),
        ));
    let server_conn = Arc::new(Connection::new(server_transport));
    (client_transport, server_conn)
}

#[tokio::test]
async fn initialize_sends_correct_request() {
    let (transport, server) = make_pair();
    let mut server_rx = server.start();

    let client = AgentClient::new(transport);

    let server_clone = server.clone();
    tokio::spawn(async move {
        if let Some(Incoming::Request { id, method, .. }) = server_rx.recv().await {
            assert_eq!(method, "initialize");
            let _ = server_clone
                .respond(id, serde_json::json!({"protocol_version": 1}))
                .await;
        }
    });

    let result = client.initialize().await.unwrap();
    assert_eq!(result["protocol_version"], 1);
}

#[tokio::test]
async fn recv_delivers_agent_events() {
    let (transport, server) = make_pair();
    let _server_rx = server.start();
    let mut client = AgentClient::new(transport);

    // Server sends event notification
    server
        .send_notification(
            methods::AGENT_EVENT.name,
            serde_json::json!({
                "agent_name": null,
                "payload": {"Stream": {"text": "hi"}}
            }),
        )
        .await
        .unwrap();

    let event = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client.recv(),
    )
    .await
    .unwrap();

    match event {
        Some(loopal_agent_client::AgentClientEvent::AgentEvent(ev)) => {
            match ev.payload {
                loopal_protocol::AgentEventPayload::Stream { text } => {
                    assert_eq!(text, "hi");
                }
                _ => panic!("expected Stream event"),
            }
        }
        other => panic!("expected AgentEvent, got: {other:?}"),
    }
}

#[tokio::test]
async fn recv_delivers_permission_request() {
    let (transport, server) = make_pair();
    let _server_rx = server.start();
    let mut client = AgentClient::new(transport);

    // Server sends permission request in background (it awaits response)
    let server_clone = server.clone();
    let server_handle = tokio::spawn(async move {
        server_clone
            .send_request(
                methods::AGENT_PERMISSION.name,
                serde_json::json!({
                    "tool_call_id": "tc1",
                    "tool_name": "Bash",
                    "tool_input": {"command": "ls"},
                }),
            )
            .await
    });

    let event = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client.recv(),
    )
    .await
    .unwrap();

    match event {
        Some(loopal_agent_client::AgentClientEvent::PermissionRequest { id, params }) => {
            assert_eq!(params["tool_name"], "Bash");
            client.respond_permission(id, true).await.unwrap();
        }
        other => panic!("expected PermissionRequest, got: {other:?}"),
    }

    // Server should have received the response
    let resp = server_handle.await.unwrap().unwrap();
    assert_eq!(resp["allow"], true);
}

#[tokio::test]
async fn into_parts_transfers_connection() {
    let (transport, server) = make_pair();
    let _server_rx = server.start();
    let client = AgentClient::new(transport);

    let (conn, mut rx) = client.into_parts();

    // Connection still works for sending
    conn.send_notification("test", serde_json::json!(null))
        .await
        .unwrap();

    // Server sends something back
    server
        .send_notification("reply", serde_json::json!({"ok": true}))
        .await
        .unwrap();

    // Incoming channel still delivers
    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .unwrap();
    assert!(msg.is_some());
}

