use crate::SpawnRegistry;
use crate::types::AgentExecutionRef;

#[test]
fn default_registry_resolves_exact_roots_and_rejects_stale_leases() {
    let registry = SpawnRegistry::default();
    let root_dir = tempfile::tempdir().unwrap();
    let child_dir = tempfile::tempdir().unwrap();
    let root = AgentExecutionRef::local("root", 3);
    let child = AgentExecutionRef::local("child", 7);
    assert!(registry.register_exact(root.clone(), root_dir.path().into(), None));
    assert!(registry.register_exact(child.clone(), child_dir.path().into(), Some(root.clone()),));

    assert!(!registry.is_root("missing"));
    assert!(registry.is_root("root"));
    assert!(!registry.is_root("child"));
    assert_eq!(registry.root_of("child").as_deref(), Some("root"));
    assert_eq!(registry.root_execution(&child), Some(root.clone()));
    assert_eq!(
        registry.root_execution(&AgentExecutionRef::local("child", 6)),
        None
    );
    assert!(
        registry
            .cwd_for(&AgentExecutionRef::local("child", 6))
            .is_none()
    );
    assert!(!registry.register_exact(
        AgentExecutionRef::local("child", 6),
        child_dir.path().into(),
        Some(root),
    ));
}

#[test]
fn vault_access_rejects_missing_callers_and_nonexistent_targets() {
    let registry = SpawnRegistry::default();
    let root_dir = tempfile::tempdir().unwrap();
    let root = AgentExecutionRef::local("root", 1);
    assert!(registry.register_exact(root.clone(), root_dir.path().into(), None));
    let missing_target = root_dir.path().join("does-not-exist");

    assert!(!registry.verify_vault_access("missing", root_dir.path()));
    assert!(!registry.verify_vault_access("root", &missing_target));
    assert!(
        !registry
            .verify_vault_access_exact(&AgentExecutionRef::local("missing", 1), root_dir.path(),)
    );
    assert!(registry.verify_vault_access_exact(&root, root_dir.path()));
}
