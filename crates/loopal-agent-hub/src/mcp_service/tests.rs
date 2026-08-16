use super::*;
use crate::spawn_registry::SpawnRegistry;
use crate::types::AgentExecutionRef;

#[tokio::test]
async fn provider_for_returns_same_instance_for_same_cwd() {
    let service = HubMcpService::new();
    let temp = tempfile::tempdir().unwrap();
    let first = service.provider_for(temp.path()).await;
    let second = service.provider_for(temp.path()).await;
    assert!(Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn provider_for_returns_different_instances_for_different_cwds() {
    let service = HubMcpService::new();
    let first = service
        .provider_for(tempfile::tempdir().unwrap().path())
        .await;
    let second = service
        .provider_for(tempfile::tempdir().unwrap().path())
        .await;
    assert!(!Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn root_of_returns_none_without_registry() {
    let service = HubMcpService::new();
    assert!(
        service
            .root_of(&AgentExecutionRef::local("anyone", 1))
            .is_none()
    );
}

fn write_settings(dir: &std::path::Path, sharing_kind: &str) {
    let loopal_dir = dir.join(".loopal");
    std::fs::create_dir_all(&loopal_dir).unwrap();
    std::fs::write(
        loopal_dir.join("settings.json"),
        format!(
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
        ),
    )
    .unwrap();
}

#[tokio::test]
async fn per_agent_provider_isolated_by_exact_execution() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    write_settings(&cwd, "per-agent");
    let first = AgentExecutionRef::local("agent-a", 1);
    let second = AgentExecutionRef::local("agent-b", 2);
    let registry = Arc::new(SpawnRegistry::new());
    assert!(registry.register_exact(first.clone(), cwd.clone(), None));
    assert!(registry.register_exact(second.clone(), cwd.clone(), None));
    let service = HubMcpService::new().with_spawn_registry(registry);

    service.on_agent_attach(first.clone(), cwd.clone()).await;
    service.on_agent_attach(second.clone(), cwd.clone()).await;

    let providers = service.per_agent.read().await;
    assert!(providers.contains_key(&first));
    assert!(providers.contains_key(&second));
    assert!(!Arc::ptr_eq(
        providers.get(&first).unwrap(),
        providers.get(&second).unwrap(),
    ));
}

#[tokio::test]
async fn stale_detach_does_not_remove_same_name_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    write_settings(&cwd, "per-agent");
    let stale = AgentExecutionRef::local("agent", 1);
    let current = AgentExecutionRef::local("agent", 2);
    let registry = Arc::new(SpawnRegistry::new());
    assert!(registry.register_exact(stale.clone(), cwd.clone(), None));
    let service = HubMcpService::new().with_spawn_registry(registry.clone());
    service.on_agent_attach(stale.clone(), cwd.clone()).await;
    assert!(registry.register_exact(current.clone(), cwd.clone(), None));
    service.on_agent_attach(current.clone(), cwd).await;

    service.on_agent_detach(&stale).await;

    let providers = service.per_agent.read().await;
    assert!(!providers.contains_key(&stale));
    assert!(providers.contains_key(&current));
}

#[tokio::test]
async fn cwd_isolation_config_flows_to_injected_args() {
    use crate::mcp_service::cwd_isolation::inject;
    use crate::mcp_service::factory::load_servers_by_sharing;
    use loopal_config::McpSharing;

    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let settings = r#"{
        "mcp_servers": {"chrome": {
            "type": "stdio", "command": "/usr/bin/true",
            "args": [], "timeout_ms": 1000, "sharing": "hub-singleton",
            "cwd_isolation": {"arg": "--user-data-dir", "cache_subdir": "isolated"}
        }}
    }"#;
    for dir in [first.path(), second.path()] {
        std::fs::create_dir_all(dir.join(".loopal")).unwrap();
        std::fs::write(dir.join(".loopal/settings.json"), settings).unwrap();
    }
    let injected_args = |dir: &std::path::Path| {
        let (name, config) = load_servers_by_sharing(dir, McpSharing::HubSingleton)
            .into_iter()
            .next()
            .unwrap();
        match inject(&name, config, dir) {
            loopal_config::McpServerConfig::Stdio { args, .. } => args,
            _ => panic!("expected stdio"),
        }
    };
    let first_args = injected_args(first.path());
    let second_args = injected_args(second.path());
    assert_ne!(first_args, second_args);
    assert!(first_args.iter().any(|arg| arg.contains("isolated")));
}
