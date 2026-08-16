use std::sync::Arc;

use super::HubMcpService;
use crate::spawn_registry::SpawnRegistry;
use crate::types::AgentExecutionRef;

#[tokio::test]
async fn default_matches_new_empty_state() {
    for service in [HubMcpService::new(), HubMcpService::default()] {
        assert!(service.hub_singleton.read().await.is_empty());
        assert!(service.per_agent.read().await.is_empty());
        assert!(service.spawn_tree.read().await.is_empty());
        assert!(service.vault_service.is_none());
        assert!(service.spawn_registry.is_none());
    }
}

#[tokio::test]
async fn with_vault_service_is_retained_by_provider_factory() {
    let (temp, vault) = super::test_vault::service(&[("token", "secret-value")]).await;
    let service = HubMcpService::new().with_vault_service(vault.clone());
    assert!(Arc::ptr_eq(service.vault_service.as_ref().unwrap(), &vault));

    let provider = service.build_hub_singleton(temp.path()).await;
    assert!(
        provider
            .manager()
            .read()
            .await
            .prepare_connections(&indexmap::IndexMap::new())
            .await
            .is_empty()
    );
}

#[test]
fn root_of_delegates_exact_generation_to_registry() {
    let temp = tempfile::tempdir().unwrap();
    let registry = Arc::new(SpawnRegistry::new());
    let root = AgentExecutionRef::local("root", 4);
    let child = AgentExecutionRef::local("child", 5);
    assert!(registry.register_exact(root.clone(), temp.path().into(), None));
    assert!(registry.register_exact(child.clone(), temp.path().into(), Some(root.clone())));
    let service = HubMcpService::new().with_spawn_registry(registry.clone());

    assert_eq!(service.root_of(&root), Some(root.clone()));
    assert_eq!(service.root_of(&child), Some(root));
    assert!(
        service
            .root_of(&AgentExecutionRef::local("child", 6))
            .is_none()
    );
}
