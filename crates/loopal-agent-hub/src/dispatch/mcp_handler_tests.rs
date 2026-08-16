use std::collections::HashMap;
use std::sync::Arc;

use loopal_config::{McpServerConfig, McpSharing};
use loopal_ipc::Connection;
use loopal_mcp::{ConnectionStatus, LocalMcpProvider, McpConnection, McpManager};
use loopal_tool_api::ToolDefinition;
use tokio::sync::{Mutex, RwLock, mpsc};

use super::{
    handle_mcp_call_tool, handle_mcp_list_tools, handle_mcp_reconnect, handle_mcp_snapshot,
};
use crate::request_principal::AgentPrincipal;
use crate::types::{AgentRuntimeFacts, SpawnAuthority};
use crate::{Hub, HubMcpService};

async fn fixture() -> (Arc<Mutex<Hub>>, AgentPrincipal) {
    let (events, _rx) = mpsc::channel(8);
    let hub = Arc::new(Mutex::new(Hub::new(events)));
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let connection = Connection::new(transport).into_listening().0;
    let mut locked = hub.lock().await;
    let execution = locked
        .registry
        .register_connection_with_parent_execution("agent", connection, None, None, None)
        .unwrap();
    let facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    assert!(locked.registry.set_runtime_facts(&execution, facts.clone()));
    let principal = AgentPrincipal::new(execution, facts);
    drop(locked);
    (hub, principal)
}

fn provider() -> Arc<LocalMcpProvider> {
    let config = McpServerConfig::Stdio {
        command: "fixture".into(),
        args: Vec::new(),
        env: HashMap::new(),
        enabled: true,
        timeout_ms: 10,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    };
    let mut connection = McpConnection::new("server".into(), config, None);
    connection.status = ConnectionStatus::Connected;
    connection.cached_tools.push(ToolDefinition {
        name: "lookup".into(),
        description: "Lookup data".into(),
        input_schema: serde_json::json!({"type": "object"}),
    });
    let mut manager = McpManager::new();
    manager.absorb_connections(vec![connection]).unwrap();
    Arc::new(LocalMcpProvider::new(Arc::new(RwLock::new(manager))))
}

#[tokio::test]
async fn handlers_project_tools_snapshots_and_stale_generation() {
    let (hub, principal) = fixture().await;
    let cwd = principal
        .cwd
        .canonicalize()
        .unwrap_or(principal.cwd.clone());
    let registry = hub.lock().await.spawn_registry.clone();
    assert!(registry.register_exact(principal.execution.clone(), cwd.clone(), None));
    let service = Arc::new(HubMcpService::new().with_spawn_registry(registry));
    service.hub_singleton.write().await.insert(cwd, provider());
    hub.lock().await.set_mcp_service(service);

    let tools = handle_mcp_list_tools(&hub, &principal).await.unwrap();
    assert_eq!(tools["tools"][0]["name"], "lookup");
    let snapshot = handle_mcp_snapshot(&hub, &principal).await.unwrap();
    assert_eq!(snapshot["servers"][0]["name"], "server");
    let reconnect = handle_mcp_reconnect(&hub, serde_json::json!({"server": "server"}), &principal)
        .await
        .unwrap();
    assert_eq!(reconnect["connected"], false);
    assert!(
        handle_mcp_reconnect(&hub, serde_json::json!({"server": "missing"}), &principal)
            .await
            .unwrap_err()
            .contains("no provider")
    );
    let call = handle_mcp_call_tool(
        &hub,
        serde_json::json!({"server": "server", "tool": "lookup", "args": {}}),
        &principal,
    )
    .await
    .unwrap_err();
    assert!(call.contains("call_tool"));

    assert!(
        handle_mcp_reconnect(&hub, serde_json::Value::Null, &principal)
            .await
            .unwrap_err()
            .contains("invalid reconnect params")
    );
    hub.lock().await.registry.unregister_connection("agent");
    assert_eq!(
        handle_mcp_list_tools(&hub, &principal).await.unwrap_err(),
        "stale Agent connection"
    );
    assert_eq!(
        handle_mcp_reconnect(&hub, serde_json::json!({"server": "server"}), &principal,)
            .await
            .unwrap_err(),
        "stale Agent connection"
    );
}
