use std::collections::HashMap;
use std::sync::Arc;

use loopal_config::{McpServerConfig, McpSharing};
use loopal_mcp::{ConnectionStatus, LocalMcpProvider, McpConnection, McpManager};
use tokio::sync::RwLock;

use super::*;
use crate::spawn_registry::SpawnRegistry;

fn provider(server: &str) -> Arc<LocalMcpProvider> {
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
    connection.status = ConnectionStatus::Failed("initial".into());
    let mut manager = McpManager::new();
    let _ = manager.absorb_connections(vec![connection]);
    Arc::new(LocalMcpProvider::new(Arc::new(RwLock::new(manager))))
}

#[tokio::test]
async fn owner_priority_includes_failed_zero_tool_servers() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().canonicalize().unwrap();
    let registry = Arc::new(SpawnRegistry::new());
    let root = AgentExecutionRef::local("root", 1);
    let child = AgentExecutionRef::local("child", 2);
    assert!(registry.register_exact(root.clone(), cwd.clone(), None));
    assert!(registry.register_exact(child.clone(), cwd.clone(), Some(root.clone())));
    let service = HubMcpService::new().with_spawn_registry(registry);
    let agent = provider("server");
    let tree = provider("server");
    let singleton = provider("server");
    service
        .per_agent
        .write()
        .await
        .insert(child.clone(), agent.clone());
    service
        .spawn_tree
        .write()
        .await
        .insert(root.clone(), tree.clone());
    service
        .hub_singleton
        .write()
        .await
        .insert(cwd.clone(), singleton.clone());

    let selected = service
        .provider_owning(&child, &cwd, "server")
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&selected, &agent));
    service.per_agent.write().await.remove(&child);
    let selected = service
        .provider_owning(&child, &cwd, "server")
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&selected, &tree));
    service.spawn_tree.write().await.remove(&root);
    let selected = service
        .provider_owning(&child, &cwd, "server")
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&selected, &singleton));
    assert!(
        service
            .provider_owning(&child, &cwd, "missing")
            .await
            .is_none()
    );
}

#[tokio::test]
async fn reconnect_requires_exact_topology_registry() {
    let temp = tempfile::tempdir().unwrap();
    let cwd = temp.path().canonicalize().unwrap();
    let execution = AgentExecutionRef::local("root", 1);
    let service = HubMcpService::new();
    service
        .hub_singleton
        .write()
        .await
        .insert(cwd.clone(), provider("server"));

    assert_eq!(
        service.reconnect_for(&execution, &cwd, "server").await,
        None
    );
    assert_eq!(
        service.reconnect_for(&execution, &cwd, "missing").await,
        None
    );
}
