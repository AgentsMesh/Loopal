use super::*;
use crate::spawn_registry::SpawnRegistry;
use crate::types::AgentExecutionRef;

fn write_settings(dir: &std::path::Path) {
    let loopal_dir = dir.join(".loopal");
    std::fs::create_dir_all(&loopal_dir).unwrap();
    std::fs::write(
        loopal_dir.join("settings.json"),
        r#"{
            "mcp_servers": {"test-server": {
                "type": "stdio", "command": "/usr/bin/true", "args": [],
                "timeout_ms": 1000, "sharing": "spawn-tree"
            }}
        }"#,
    )
    .unwrap();
}

fn exact_tree(
    cwd: &std::path::Path,
    root_generation: u64,
) -> (Arc<SpawnRegistry>, AgentExecutionRef, AgentExecutionRef) {
    let registry = Arc::new(SpawnRegistry::new());
    let root = AgentExecutionRef::local("root", root_generation);
    let child = AgentExecutionRef::local("child", root_generation.saturating_add(1));
    assert!(registry.register_exact(root.clone(), cwd.to_path_buf(), None));
    assert!(registry.register_exact(child.clone(), cwd.to_path_buf(), Some(root.clone()),));
    (registry, root, child)
}

#[tokio::test]
async fn spawn_tree_owned_by_exact_root_and_shared_with_child() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    write_settings(&cwd);
    let (registry, root, child) = exact_tree(&cwd, 1);
    let service = HubMcpService::new().with_spawn_registry(registry);

    service.on_agent_attach(root.clone(), cwd.clone()).await;
    assert!(service.spawn_tree.read().await.contains_key(&root));
    let count = service.spawn_tree.read().await.len();
    service.on_agent_attach(child, cwd).await;
    assert_eq!(service.spawn_tree.read().await.len(), count);
}

#[tokio::test]
async fn child_detach_does_not_drop_root_provider() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    write_settings(&cwd);
    let (registry, root, child) = exact_tree(&cwd, 3);
    let service = HubMcpService::new().with_spawn_registry(registry);
    service.on_agent_attach(root.clone(), cwd.clone()).await;
    service.on_agent_attach(child.clone(), cwd).await;

    service.on_agent_detach(&child).await;
    assert!(service.spawn_tree.read().await.contains_key(&root));
    service.on_agent_detach(&root).await;
    assert!(!service.spawn_tree.read().await.contains_key(&root));
}

#[tokio::test]
async fn stale_root_detach_cannot_remove_replacement_generation() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    write_settings(&cwd);
    let registry = Arc::new(SpawnRegistry::new());
    let stale = AgentExecutionRef::local("root", 1);
    let current = AgentExecutionRef::local("root", 2);
    assert!(registry.register_exact(stale.clone(), cwd.clone(), None));
    let service = HubMcpService::new().with_spawn_registry(registry.clone());
    service.on_agent_attach(stale.clone(), cwd.clone()).await;
    assert!(registry.register_exact(current.clone(), cwd.clone(), None));
    service.on_agent_attach(current.clone(), cwd).await;

    service.on_agent_detach(&stale).await;

    let providers = service.spawn_tree.read().await;
    assert!(!providers.contains_key(&stale));
    assert!(providers.contains_key(&current));
}

#[tokio::test]
async fn snapshots_include_exact_spawn_tree_provider_for_child() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    write_settings(&cwd);
    let (registry, root, child) = exact_tree(&cwd, 10);
    let service = HubMcpService::new().with_spawn_registry(registry);
    service.on_agent_attach(root.clone(), cwd.clone()).await;
    let provider = service.spawn_tree.read().await[&root].clone();
    provider
        .wait_until_settled(std::time::Duration::from_secs(2))
        .await;

    let snapshots = service.snapshots_for(&child, &cwd).await;
    assert!(snapshots.iter().any(|server| server.name == "test-server"));
}
