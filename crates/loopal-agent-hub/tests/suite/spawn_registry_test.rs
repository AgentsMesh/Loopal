use loopal_agent_hub::SpawnRegistry;
use std::path::{Path, PathBuf};

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn register_and_lookup_cwd() {
    let r = SpawnRegistry::new();
    let dir = tmp();
    r.register("a1".into(), dir.path().to_path_buf(), None);
    assert_eq!(r.cwd_of("a1"), Some(dir.path().canonicalize().unwrap()));
}

#[test]
fn unregister_removes_entry() {
    let r = SpawnRegistry::new();
    let dir = tmp();
    r.register("a1".into(), dir.path().to_path_buf(), None);
    assert!(r.unregister("a1"));
    assert_eq!(r.cwd_of("a1"), None);
    assert!(!r.unregister("a1"));
}

#[test]
fn verify_self_access_allowed() {
    let r = SpawnRegistry::new();
    let dir = tmp();
    r.register("a1".into(), dir.path().to_path_buf(), None);
    assert!(r.verify_vault_access("a1", dir.path()));
}

#[test]
fn verify_descendant_cwd_allowed() {
    let r = SpawnRegistry::new();
    let root = tmp();
    let sub = root.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    r.register("a1".into(), root.path().to_path_buf(), None);
    assert!(r.verify_vault_access("a1", &sub));
}

#[test]
fn verify_ancestor_cwd_allowed() {
    let r = SpawnRegistry::new();
    let root = tmp();
    let sub = root.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    r.register("a1".into(), sub.clone(), None);
    assert!(r.verify_vault_access("a1", root.path()));
}

#[test]
fn verify_sibling_cwd_denied() {
    let r = SpawnRegistry::new();
    let root = tmp();
    let a = root.path().join("a");
    let b = root.path().join("b");
    std::fs::create_dir(&a).unwrap();
    std::fs::create_dir(&b).unwrap();
    r.register("a1".into(), a, None);
    assert!(!r.verify_vault_access("a1", &b));
}

#[test]
fn verify_sub_agent_walks_to_root() {
    let r = SpawnRegistry::new();
    let root = tmp();
    let sub = root.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    r.register("root".into(), root.path().to_path_buf(), None);
    r.register("child".into(), sub.clone(), Some("root".into()));
    assert!(r.verify_vault_access("child", root.path()));
}

#[test]
fn verify_unknown_caller_denied() {
    let r = SpawnRegistry::new();
    let dir = tmp();
    assert!(!r.verify_vault_access("ghost", dir.path()));
}

#[test]
fn verify_nonexistent_target_denied() {
    let r = SpawnRegistry::new();
    let dir = tmp();
    r.register("a1".into(), dir.path().to_path_buf(), None);
    assert!(!r.verify_vault_access("a1", Path::new("/nonexistent/xyz/qwe")));
}

#[test]
fn parent_chain_cycle_is_bounded() {
    let r = SpawnRegistry::new();
    let dir = tmp();
    let cwd = PathBuf::from(dir.path());
    r.register("a".into(), cwd.clone(), Some("b".into()));
    r.register("b".into(), cwd, Some("a".into()));
    assert!(!r.verify_vault_access("a", dir.path()));
    assert!(r.root_of("a").is_none());
}

#[test]
fn root_of_distinguishes_unknown_from_cycle_via_warn() {
    // Both unknown agent and cycle return None from root_of; verify_vault_access
    // is the right place to consume the WalkError detail. Unknown caller is denied
    // (NotFound); cycle is also denied (CycleDetected) but emits a warn.
    let r = SpawnRegistry::new();
    let dir = tmp();
    assert!(r.root_of("never_registered").is_none());

    let cwd = PathBuf::from(dir.path());
    r.register("x".into(), cwd.clone(), Some("y".into()));
    r.register("y".into(), cwd, Some("x".into()));
    assert!(r.root_of("x").is_none());
    assert!(!r.verify_vault_access("x", dir.path()));
}

#[test]
fn root_of_returns_self_when_no_parent() {
    let r = SpawnRegistry::new();
    let dir = tmp();
    r.register("root".into(), dir.path().to_path_buf(), None);
    assert_eq!(r.root_of("root").as_deref(), Some("root"));
}

#[test]
fn root_of_walks_two_hops() {
    let r = SpawnRegistry::new();
    let dir = tmp();
    r.register("root".into(), dir.path().to_path_buf(), None);
    r.register("mid".into(), dir.path().to_path_buf(), Some("root".into()));
    r.register("leaf".into(), dir.path().to_path_buf(), Some("mid".into()));
    assert_eq!(r.root_of("leaf").as_deref(), Some("root"));
}

#[test]
fn root_of_unknown_agent_is_none() {
    let r = SpawnRegistry::new();
    assert!(r.root_of("ghost").is_none());
}

#[test]
fn parent_of_returns_immediate_parent() {
    let r = SpawnRegistry::new();
    let dir = tmp();
    r.register("root".into(), dir.path().to_path_buf(), None);
    r.register(
        "child".into(),
        dir.path().to_path_buf(),
        Some("root".into()),
    );
    assert_eq!(r.parent_of("child").as_deref(), Some("root"));
    assert_eq!(r.parent_of("root"), None);
}
