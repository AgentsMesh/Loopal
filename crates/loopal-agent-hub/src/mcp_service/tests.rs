use super::*;
use crate::spawn_registry::SpawnRegistry;

#[tokio::test]
async fn provider_for_returns_same_instance_for_same_cwd() {
    let svc = HubMcpService::new();
    let tmp = tempfile::tempdir().unwrap();
    let a = svc.provider_for(tmp.path()).await;
    let b = svc.provider_for(tmp.path()).await;
    assert!(Arc::ptr_eq(&a, &b));
}

#[tokio::test]
async fn provider_for_returns_different_instances_for_different_cwds() {
    let svc = HubMcpService::new();
    let a_dir = tempfile::tempdir().unwrap();
    let b_dir = tempfile::tempdir().unwrap();
    let a = svc.provider_for(a_dir.path()).await;
    let b = svc.provider_for(b_dir.path()).await;
    assert!(!Arc::ptr_eq(&a, &b));
}

#[tokio::test]
async fn root_of_returns_none_without_registry() {
    let svc = HubMcpService::new();
    assert!(svc.root_of("anyone").is_none());
}

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
    assert!(
        svc.spawn_tree.read().await.contains_key("root"),
        "root attach must provision spawn-tree owner entry"
    );
    let after_root = svc.spawn_tree.read().await.len();

    svc.on_agent_attach("child".into(), canonical.clone(), Some("root".into()))
        .await;
    assert_eq!(
        svc.spawn_tree.read().await.len(),
        after_root,
        "sub-agent attach must NOT create a second spawn-tree entry"
    );
    assert!(
        svc.spawn_tree.read().await.contains_key("root"),
        "spawn-tree entry remains keyed by root, not child"
    );
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

#[tokio::test]
async fn was_root_false_must_not_drop_spawn_tree_even_for_root_agent() {
    // Reason: was_root is the authoritative signal — even if agent_name
    // matches the root, was_root=false must NOT drop. Prevents detach-order
    // races from taking down spawn-tree while sub-agents are still alive.
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
    assert!(
        svc.spawn_tree.read().await.contains_key("root"),
        "was_root=false must prevent drop, regardless of agent_name"
    );
}

#[tokio::test]
async fn per_agent_provider_isolated_per_agent() {
    let dir = tempfile::tempdir().unwrap();
    let canonical = dir.path().canonicalize().unwrap();
    write_settings(&canonical, "per-agent");

    let registry = Arc::new(SpawnRegistry::new());
    registry.register("agent-a".into(), canonical.clone(), None);
    registry.register("agent-b".into(), canonical.clone(), None);

    let svc = HubMcpService::new().with_spawn_registry(registry);
    svc.on_agent_attach("agent-a".into(), canonical.clone(), None)
        .await;
    svc.on_agent_attach("agent-b".into(), canonical.clone(), None)
        .await;

    let per_agent = svc.per_agent.read().await;
    assert!(per_agent.contains_key("agent-a"));
    assert!(per_agent.contains_key("agent-b"));
    assert!(
        !Arc::ptr_eq(
            per_agent.get("agent-a").unwrap(),
            per_agent.get("agent-b").unwrap(),
        ),
        "per-agent providers must be distinct instances per agent"
    );
}

#[tokio::test]
async fn cwd_isolation_config_flows_from_settings_to_injected_args() {
    use crate::mcp_service::cwd_isolation::inject;
    use crate::mcp_service::factory::load_servers_by_sharing;
    use loopal_config::McpSharing;

    let dir_a = tempfile::tempdir().unwrap();
    let dir_b = tempfile::tempdir().unwrap();
    let canonical_a = dir_a.path().canonicalize().unwrap();
    let canonical_b = dir_b.path().canonicalize().unwrap();
    let settings = r#"{
        "mcp_servers": {
            "chrome-mock": {
                "type": "stdio",
                "command": "/usr/bin/true",
                "args": ["-y", "chrome-devtools-mcp@latest"],
                "timeout_ms": 1000,
                "sharing": "hub-singleton",
                "cwd_isolation": {
                    "arg": "--user-data-dir",
                    "cache_subdir": "chrome-isolated"
                }
            }
        }
    }"#;
    for d in [&canonical_a, &canonical_b] {
        let loopal = d.join(".loopal");
        std::fs::create_dir_all(&loopal).unwrap();
        std::fs::write(loopal.join("settings.json"), settings).unwrap();
    }

    let servers_a = load_servers_by_sharing(&canonical_a, McpSharing::HubSingleton);
    assert_eq!(
        servers_a.len(),
        1,
        "config must yield one hub-singleton server"
    );
    let (name_a, cfg_a) = servers_a.into_iter().next().unwrap();
    assert!(
        cfg_a.cwd_isolation().is_some(),
        "cwd_isolation must deserialize"
    );

    let injected_a = inject(&name_a, cfg_a, &canonical_a);
    let injected_b = inject(
        &name_a,
        load_servers_by_sharing(&canonical_b, McpSharing::HubSingleton)
            .into_iter()
            .next()
            .unwrap()
            .1,
        &canonical_b,
    );

    let args_a = match &injected_a {
        loopal_config::McpServerConfig::Stdio { args, .. } => args.clone(),
        _ => panic!("expected stdio"),
    };
    let args_b = match &injected_b {
        loopal_config::McpServerConfig::Stdio { args, .. } => args.clone(),
        _ => panic!("expected stdio"),
    };

    let arg_a = args_a
        .iter()
        .find(|s| s.starts_with("--user-data-dir="))
        .expect("injected --user-data-dir for cwd A");
    let arg_b = args_b
        .iter()
        .find(|s| s.starts_with("--user-data-dir="))
        .expect("injected --user-data-dir for cwd B");
    assert_ne!(
        arg_a, arg_b,
        "two different cwds must get different --user-data-dir paths"
    );
    assert!(arg_a.contains("chrome-isolated"));
    assert!(arg_b.contains("chrome-isolated"));
}
