use super::*;
use crate::spawn_registry::SpawnRegistry;

fn write_settings(dir: &std::path::Path, sharing_kind: &str) {
    let loopal_dir = dir.join(".loopal");
    std::fs::create_dir_all(&loopal_dir).unwrap();
    let settings = format!(
        r#"{{
            "mcp_servers": {{
                "test-server": {{
                    "type": "stdio",
                    "command": "/usr/bin/true",
                    "args": [],
                    "timeout_ms": 1000,
                    "sharing": "{sharing_kind}"
                }}
            }}
        }}"#
    );
    std::fs::write(loopal_dir.join("settings.json"), settings).unwrap();
}

#[tokio::test]
async fn spawn_tree_owned_by_root_shared_with_sub_agents() {
    let dir = tempfile::tempdir().unwrap();
    let canonical = dir.path().canonicalize().unwrap();
    write_settings(&canonical, "spawn-tree");

    let registry = Arc::new(SpawnRegistry::new());
    registry.register("root".into(), canonical.clone(), None);
    registry.register("child".into(), canonical.clone(), Some("root".into()));

    let svc = HubMcpService::new().with_spawn_registry(registry);

    svc.on_agent_attach("root".into(), canonical.clone(), None)
        .await;
    assert!(svc.spawn_tree.read().await.contains_key("root"));
    let after_root = svc.spawn_tree.read().await.len();

    svc.on_agent_attach("child".into(), canonical.clone(), Some("root".into()))
        .await;
    assert_eq!(
        svc.spawn_tree.read().await.len(),
        after_root,
        "sub-agent attach must NOT create a second spawn-tree entry"
    );
    assert!(svc.spawn_tree.read().await.contains_key("root"));
}

#[tokio::test]
async fn spawn_tree_dropped_only_when_root_detaches() {
    let dir = tempfile::tempdir().unwrap();
    let canonical = dir.path().canonicalize().unwrap();
    write_settings(&canonical, "spawn-tree");

    let registry = Arc::new(SpawnRegistry::new());
    registry.register("root".into(), canonical.clone(), None);
    registry.register("child".into(), canonical.clone(), Some("root".into()));

    let svc = HubMcpService::new().with_spawn_registry(registry);
    svc.on_agent_attach("root".into(), canonical.clone(), None)
        .await;
    svc.on_agent_attach("child".into(), canonical.clone(), Some("root".into()))
        .await;
    assert!(svc.spawn_tree.read().await.contains_key("root"));

    svc.on_agent_detach("child", false).await;
    assert!(
        svc.spawn_tree.read().await.contains_key("root"),
        "sub-agent detach must not drop the root's spawn-tree instance"
    );

    svc.on_agent_detach("root", true).await;
    assert!(
        !svc.spawn_tree.read().await.contains_key("root"),
        "root detach must release the spawn-tree instance"
    );
}

// was_root is the authoritative signal — even if agent_name matches the root,
// was_root=false must NOT drop. Prevents detach-order races from taking down
// spawn-tree while sub-agents are still alive.
#[tokio::test]
async fn was_root_false_must_not_drop_spawn_tree_even_for_root_agent() {
    let dir = tempfile::tempdir().unwrap();
    let canonical = dir.path().canonicalize().unwrap();
    write_settings(&canonical, "spawn-tree");

    let registry = Arc::new(SpawnRegistry::new());
    registry.register("root".into(), canonical.clone(), None);
    let svc = HubMcpService::new().with_spawn_registry(registry);
    svc.on_agent_attach("root".into(), canonical.clone(), None)
        .await;
    assert!(svc.spawn_tree.read().await.contains_key("root"));

    svc.on_agent_detach("root", false).await;
    assert!(svc.spawn_tree.read().await.contains_key("root"));
}
