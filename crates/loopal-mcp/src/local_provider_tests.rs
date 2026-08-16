use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use loopal_config::{McpServerConfig, McpSharing};
use loopal_ipc::HUB_RPC_BUDGET;
use loopal_tool_api::ToolDefinition;
use tokio::sync::RwLock;

use super::LocalMcpProvider;
use crate::connection::McpConnection;
use crate::manager::McpManager;
use crate::provider::McpProvider;
use crate::types::ConnectionStatus;

fn config(command: &str) -> McpServerConfig {
    McpServerConfig::Stdio {
        command: command.into(),
        args: Vec::new(),
        env: HashMap::new(),
        enabled: true,
        timeout_ms: 10,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    }
}

fn provider_with(connection: McpConnection) -> LocalMcpProvider {
    let mut manager = McpManager::new();
    let _ = manager.absorb_connections(vec![connection]);
    LocalMcpProvider::new(Arc::new(RwLock::new(manager)))
}

#[tokio::test]
async fn exposes_manager_events_tools_and_server_presence() {
    let mut connection = McpConnection::new("server".into(), config("fixture"), None);
    connection.status = ConnectionStatus::Connected;
    connection.cached_tools.push(ToolDefinition {
        name: "lookup".into(),
        description: String::new(),
        input_schema: serde_json::Value::Null,
    });
    let provider = provider_with(connection);

    assert_eq!(provider.manager().read().await.collect_snapshots().len(), 1);
    assert_eq!(*provider.subscribe_settle_events().borrow(), 0);
    assert!(provider.owns_server("server").await);
    assert!(!provider.owns_server("missing").await);
    assert!(provider.has_server("server").await);
    assert!(!provider.has_server("missing").await);
    assert_eq!(provider.list_tools(HUB_RPC_BUDGET).await.len(), 1);
    assert_eq!(provider.snapshot(HUB_RPC_BUDGET).await.len(), 1);
}

#[tokio::test]
async fn failed_reconnect_preserves_installed_failure() {
    let mut connection = McpConnection::new(
        "server".into(),
        config("__missing_local_provider_fixture__"),
        None,
    );
    connection.status = ConnectionStatus::Failed("initial".into());
    let provider = provider_with(connection);

    assert!(!provider.try_reconnect("server").await);
    assert!(!provider.try_reconnect("missing").await);
    let manager = provider.manager.read().await;
    assert!(manager.connections["server"].status.is_failed());
    assert_eq!(manager.connections["server"].errors, Vec::<String>::new());
}

#[tokio::test]
async fn provider_reconnect_maps_missing_server_to_failure() {
    let provider = LocalMcpProvider::new(Arc::new(RwLock::new(McpManager::new())));

    let error = provider
        .reconnect("missing", HUB_RPC_BUDGET)
        .await
        .unwrap_err();

    assert!(matches!(error, loopal_error::McpError::ConnectionFailed(_)));
}

#[tokio::test]
async fn empty_background_spawn_keeps_provider_settled() {
    let provider = LocalMcpProvider::new(Arc::new(RwLock::new(McpManager::new())));
    provider.spawn_background(indexmap::IndexMap::new());
    assert!(provider.wait_until_settled(Duration::from_millis(10)).await);
    provider.await_all_settled().await;
}
