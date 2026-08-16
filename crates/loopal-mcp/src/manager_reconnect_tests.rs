use std::collections::HashMap;

use loopal_config::{McpServerConfig, McpSharing};
use loopal_tool_api::ToolDefinition;

use super::McpManager;
use crate::connection::McpConnection;
use crate::provider_call_tests::fixture_client;
use crate::types::ConnectionStatus;

fn config() -> McpServerConfig {
    McpServerConfig::Stdio {
        command: "fixture".into(),
        args: Vec::new(),
        env: HashMap::new(),
        enabled: true,
        timeout_ms: 100,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    }
}

fn failed() -> McpConnection {
    let mut connection = McpConnection::new("server".into(), config(), None);
    connection.status = ConnectionStatus::Failed("closed".into());
    connection
}

async fn connected(tool: &str) -> McpConnection {
    let client = fixture_client(serde_json::json!({}), HashMap::new()).await;
    let mut connection = McpConnection::new("server".into(), config(), None).with_client(client);
    connection.cached_tools.push(ToolDefinition {
        name: tool.into(),
        description: String::new(),
        input_schema: serde_json::Value::Null,
    });
    connection
}

#[tokio::test]
async fn stale_failed_candidate_cannot_remove_successful_generation() {
    let mut manager = McpManager::new();
    let _ = manager.absorb_connections(vec![failed()]);
    let mut winner = manager.plan_reconnect("server", None).unwrap().unwrap();
    let loser = manager.plan_reconnect("server", None).unwrap().unwrap();
    winner.candidate = connected("winner_tool").await;

    let committed = manager.commit_reconnect(winner);
    assert!(committed.usable);
    assert!(committed.retired.is_some());
    let stale = manager.commit_reconnect(loser);

    assert!(stale.usable);
    assert!(stale.retired.is_some());
    assert!(manager.connections["server"].status.is_connected());
    assert_eq!(manager.tool_map["winner_tool"], "server");
}

#[tokio::test]
async fn disconnect_generation_rejects_in_flight_successful_candidate() {
    let mut manager = McpManager::new();
    let _ = manager.absorb_connections(vec![failed()]);
    let mut plan = manager.plan_reconnect("server", None).unwrap().unwrap();
    plan.candidate = connected("stale_tool").await;
    manager.connections["server"].disconnect().await;

    let stale = manager.commit_reconnect(plan);

    assert!(!stale.usable);
    assert!(stale.retired.is_some());
    assert_eq!(
        manager.connections["server"].status,
        ConnectionStatus::Disconnected
    );
    assert!(!manager.tool_map.contains_key("stale_tool"));
}

#[tokio::test]
async fn removed_server_rejects_in_flight_candidate() {
    let mut manager = McpManager::new();
    let _ = manager.absorb_connections(vec![failed()]);
    let mut plan = manager.plan_reconnect("server", None).unwrap().unwrap();
    plan.candidate = connected("orphaned_tool").await;
    manager.connections.shift_remove("server");

    let commit = manager.commit_reconnect(plan);

    assert!(!commit.usable);
    assert!(commit.retired.is_some());
    assert!(!manager.tool_map.contains_key("orphaned_tool"));
}

#[tokio::test]
async fn open_current_generation_skips_unforced_plan() {
    let mut manager = McpManager::new();
    manager
        .absorb_connections(vec![connected("existing").await])
        .unwrap();

    assert!(manager.plan_reconnect("server", None).unwrap().is_none());
    assert!(manager.plan_reconnect("missing", None).is_err());
}

#[tokio::test]
async fn current_generation_transport_failure_forces_replacement_before_closed_flag() {
    let mut manager = McpManager::new();
    manager
        .absorb_connections(vec![connected("existing").await])
        .unwrap();
    let failed_generation = manager.connection_generation("server").unwrap();

    assert!(
        manager
            .plan_reconnect("server", Some(&failed_generation))
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn stale_transport_failure_reuses_open_replacement_generation() {
    let mut manager = McpManager::new();
    manager
        .absorb_connections(vec![connected("original").await])
        .unwrap();
    let stale_generation = manager.connection_generation("server").unwrap();
    manager
        .absorb_connections(vec![connected("replacement").await])
        .unwrap();

    assert!(
        manager
            .plan_reconnect("server", Some(&stale_generation))
            .unwrap()
            .is_none()
    );
}
