use std::collections::HashMap;
use std::sync::Arc;

use loopal_config::{McpServerConfig, McpSharing};
use loopal_mcp::{ConnectionStatus, LocalMcpProvider, McpConnection, McpManager};
use loopal_tool_api::ToolDefinition;
use tokio::sync::RwLock;

use super::HubMcpService;
use crate::spawn_registry::SpawnRegistry;
use crate::types::AgentExecutionRef;

fn provider(server: &str, tool: &str) -> Arc<LocalMcpProvider> {
    let config = McpServerConfig::Stdio {
        command: "fixture".into(),
        args: Vec::new(),
        env: HashMap::new(),
        enabled: true,
        timeout_ms: 10,
        sharing: McpSharing::HubSingleton,
        cwd_isolation: None,
    };
    let mut connection = McpConnection::new(server.into(), config, None);
    connection.status = ConnectionStatus::Connected;
    connection.cached_tools.push(ToolDefinition {
        name: tool.into(),
        description: format!("{tool} description"),
        input_schema: serde_json::json!({"type": "object"}),
    });
    let mut manager = McpManager::new();
    manager.absorb_connections(vec![connection]).unwrap();
    Arc::new(LocalMcpProvider::new(Arc::new(RwLock::new(manager))))
}

fn service(cwd: &std::path::Path) -> (HubMcpService, AgentExecutionRef, AgentExecutionRef) {
    let root = AgentExecutionRef::local("root", 1);
    let child = AgentExecutionRef::local("child", 2);
    let registry = Arc::new(SpawnRegistry::new());
    assert!(registry.register_exact(root.clone(), cwd.into(), None));
    assert!(registry.register_exact(child.clone(), cwd.into(), Some(root.clone())));
    (
        HubMcpService::new().with_spawn_registry(registry),
        root,
        child,
    )
}

#[tokio::test]
async fn query_priority_covers_agent_tree_and_singleton_providers() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().canonicalize().unwrap();
    let (service, root, child) = service(&cwd);
    service
        .per_agent
        .write()
        .await
        .insert(child.clone(), provider("agent", "a"));
    service
        .spawn_tree
        .write()
        .await
        .insert(root, provider("tree", "b"));
    service
        .hub_singleton
        .write()
        .await
        .insert(cwd.clone(), provider("hub", "c"));

    let tools = service.list_tools_for(&child, &cwd).await;
    assert_eq!(
        tools
            .iter()
            .map(|(server, _)| server.as_str())
            .collect::<Vec<_>>(),
        ["agent", "tree", "hub"]
    );
    let snapshots = service.snapshots_for(&child, &cwd).await;
    assert_eq!(
        snapshots
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["agent", "tree", "hub"]
    );
    for server in ["agent", "tree", "hub"] {
        assert!(
            service
                .provider_for_call(&child, &cwd, server)
                .await
                .is_some()
        );
    }
    assert!(
        service
            .provider_for_call(&child, &cwd, "missing")
            .await
            .is_none()
    );
}

#[tokio::test]
async fn local_provider_requires_canonical_existing_cwd() {
    let service = HubMcpService::new();
    let temp = tempfile::tempdir().unwrap();
    assert!(service.local_provider(temp.path()).await.is_none());
    service.provider_for(temp.path()).await;
    assert!(service.local_provider(temp.path()).await.is_some());
    assert!(
        service
            .local_provider(&temp.path().join("missing"))
            .await
            .is_none()
    );
}
