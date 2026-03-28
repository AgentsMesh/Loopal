//! McpConnection lifecycle tests.

use std::time::Duration;

use loopal_config::McpServerConfig;
use loopal_mcp::McpClient;
use loopal_mcp::connection::McpConnection;
use loopal_mcp::types::ConnectionStatus;
use loopal_test_support::mcp_mock::MockMcpServer;
use serde_json::json;

fn stdio_config() -> McpServerConfig {
    // This config will fail (no such command), used for failure path tests.
    McpServerConfig::Stdio {
        command: "__nonexistent_mcp_server__".to_string(),
        args: vec![],
        env: Default::default(),
        enabled: true,
        timeout_ms: 5000,
    }
}

#[test]
fn test_new_is_disconnected() {
    let conn = McpConnection::new("test".into(), stdio_config(), None);
    assert_eq!(conn.status, ConnectionStatus::Disconnected);
    assert!(conn.cached_tools.is_empty());
    assert!(conn.cached_resources.is_empty());
    assert!(conn.cached_prompts.is_empty());
    assert!(conn.instructions.is_none());
    assert!(conn.errors.is_empty());
    assert!(conn.client().is_none());
}

#[tokio::test]
async fn test_connect_failure_sets_failed_status() {
    let mut conn = McpConnection::new("bad".into(), stdio_config(), None);
    conn.connect().await;
    assert!(conn.status.is_failed());
    assert!(!conn.errors.is_empty());
    assert!(conn.client().is_none());
}

#[tokio::test]
async fn test_disconnect_clears_state() {
    let mut conn = McpConnection::new("test".into(), stdio_config(), None);
    // Even after a failed connect, disconnect should reset cleanly.
    conn.connect().await;
    conn.disconnect().await;
    assert_eq!(conn.status, ConnectionStatus::Disconnected);
    assert!(conn.cached_tools.is_empty());
    assert!(conn.instructions.is_none());
    assert!(conn.client().is_none());
}

/// Helper: create a connection backed by in-memory mock.
/// We bypass McpConnection::connect() since it needs real transport config,
/// and test the McpClient integration directly instead.
async fn make_connected_client() -> McpClient {
    let server = MockMcpServer::new().add_tool("test_tool", "A test tool", json!("ok"));
    let (read, write) = server.start();
    McpClient::connect((read, write), Duration::from_secs(5), None)
        .await
        .expect("mock connect failed")
}

#[tokio::test]
async fn test_mock_client_has_peer_info() {
    let client = make_connected_client().await;
    let info = client.peer_info().expect("peer info missing");
    assert_eq!(info.server_info.name, "mock");
}

#[tokio::test]
async fn test_mock_client_discovers_tools() {
    let client = make_connected_client().await;
    let tools = client.list_tools().await.expect("list_tools failed");
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.tools[0].name, "test_tool");
}
