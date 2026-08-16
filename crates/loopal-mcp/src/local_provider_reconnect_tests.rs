use std::collections::HashMap;

use loopal_config::{McpServerConfig, McpSharing};

use super::super::LocalMcpProvider;
use crate::connection::McpConnection;
use crate::manager::McpManager;
use crate::types::ConnectionStatus;

fn config(url: String) -> McpServerConfig {
    McpServerConfig::StreamableHttp {
        url,
        headers: HashMap::new(),
        enabled: true,
        timeout_ms: 2_000,
        sharing: McpSharing::HubSingleton,
    }
}

async fn provider() -> LocalMcpProvider {
    let initialize = serde_json::json!({
        "jsonrpc": "2.0", "id": "$REQUEST_ID",
        "result": {
            "protocolVersion": "2025-03-26",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "fixture", "version": "1"}
        }
    })
    .to_string()
    .replace("\"$REQUEST_ID\"", "$REQUEST_ID");
    let tools = serde_json::json!({
        "jsonrpc": "2.0", "id": "$REQUEST_ID",
        "result": {"tools": [{
            "name": "recovered_tool",
            "description": "recovered",
            "inputSchema": {"type": "object"}
        }]}
    })
    .to_string()
    .replace("\"$REQUEST_ID\"", "$REQUEST_ID");
    let (url, _) = crate::http_test_support::server(vec![
        crate::http_test_support::response("200 OK", "application/json", &initialize),
        crate::http_test_support::response("202 Accepted", "application/json", ""),
        crate::http_test_support::response("200 OK", "application/json", &tools),
        crate::http_test_support::response("202 Accepted", "application/json", ""),
    ])
    .await;
    let mut connection = McpConnection::new("server".into(), config(url), None);
    connection.status = ConnectionStatus::Failed("initial".into());
    let mut manager = McpManager::new();
    let _ = manager.absorb_connections(vec![connection]);
    LocalMcpProvider::new(std::sync::Arc::new(tokio::sync::RwLock::new(manager)))
}

#[tokio::test]
async fn guarded_reconnect_commits_once_when_authorized() {
    let provider = provider().await;
    let mut calls = 0;

    assert!(
        provider
            .try_reconnect_guarded("server", |commit| {
                commit();
                commit();
                calls += 1;
            })
            .await
    );
    assert_eq!(calls, 1);
    let manager = provider.manager.read().await;
    assert!(manager.connections["server"].status.is_connected());
    assert_eq!(manager.tool_map["recovered_tool"], "server");
}

#[tokio::test]
async fn guarded_reconnect_retires_candidate_when_authority_is_stale() {
    let provider = provider().await;

    assert!(!provider.try_reconnect_guarded("server", |_commit| {}).await);
    let manager = provider.manager.read().await;
    assert!(manager.connections["server"].status.is_failed());
    assert!(!manager.tool_map.contains_key("recovered_tool"));
}
