use std::sync::Arc;

use loopal_agent_hub::Hub;
use loopal_agent_hub::spawn_manager::register_agent_connection;
use loopal_ipc::Connection;
use loopal_ipc::protocol::methods;
use loopal_protocol::{AgentEvent, McpListToolsResponse, McpSnapshotResponse};
use serde_json::json;
use tokio::sync::{Mutex, mpsc};

async fn agent() -> (
    Arc<Mutex<Hub>>,
    Arc<Connection<loopal_ipc::Listening>>,
    mpsc::Receiver<loopal_ipc::connection::Incoming>,
) {
    let (event_tx, mut event_rx) = mpsc::channel::<AgentEvent>(16);
    tokio::spawn(async move { while event_rx.recv().await.is_some() {} });
    let hub = Arc::new(Mutex::new(Hub::new(event_tx)));
    let (client_transport, server_transport) = loopal_ipc::duplex_pair();
    let (client, client_rx) = Connection::new(client_transport).into_listening();
    let (server, server_rx) = Connection::new(server_transport).into_listening();
    register_agent_connection(hub.clone(), "child", server, server_rx, None, None, None)
        .await
        .unwrap();
    (hub, client, client_rx)
}

#[tokio::test]
async fn empty_list_and_snapshot_roundtrip_through_agent_dispatcher() {
    let (_hub, client, _client_rx) = agent().await;

    let tools: McpListToolsResponse = serde_json::from_value(
        client
            .send_request(methods::HUB_MCP_LIST_TOOLS.name, json!(null))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(tools.tools.is_empty());

    let snapshot: McpSnapshotResponse = serde_json::from_value(
        client
            .send_request(methods::HUB_MCP_SNAPSHOT.name, json!(null))
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(snapshot.servers.is_empty());

    let reconnect = client
        .send_request(
            methods::HUB_MCP_RECONNECT.name,
            json!({"server": "missing"}),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(reconnect.contains("not authorized"));
}

#[tokio::test]
async fn malformed_and_unknown_calls_fail_without_exposing_placeholders() {
    let (_hub, client, _client_rx) = agent().await;

    let malformed = client
        .send_request(
            methods::HUB_MCP_CALL_TOOL.name,
            json!({"server": "missing"}),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(malformed.contains("invalid call_tool params"));

    let unknown = client
        .send_request(
            methods::HUB_MCP_CALL_TOOL.name,
            json!({"server": "missing", "tool": "run", "args": {"value": "safe"}}),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(unknown.contains("no provider found for server 'missing'"));

    let malformed_reconnect = client
        .send_request(methods::HUB_MCP_RECONNECT.name, json!(null))
        .await
        .unwrap_err()
        .to_string();
    assert!(malformed_reconnect.contains("not authorized"));

    let rejected = client
        .send_request(
            methods::HUB_MCP_CALL_TOOL.name,
            json!({
                "server": "missing",
                "tool": "run",
                "args": {"token": "<secret_ref:private_name>"}
            }),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(!rejected.contains("private_name"));
    assert!(rejected.contains("secret"));
}
