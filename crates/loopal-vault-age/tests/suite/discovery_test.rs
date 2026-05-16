//! `list_initialized_vaults` is the single source of truth for "which vaults
//! exist on disk" — CLI list, runtime auto-discovery, and config build all use it.

use loopal_vault_age::list_initialized_vaults;
use tempfile::tempdir;

fn touch(path: &std::path::Path) {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(path, b"").unwrap();
}

#[test]
fn returns_empty_when_dir_missing() {
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("does_not_exist");
    assert!(list_initialized_vaults(&missing).is_empty());
}

#[test]
fn returns_empty_when_dir_present_but_empty() {
    let tmp = tempdir().unwrap();
    assert!(list_initialized_vaults(tmp.path()).is_empty());
}

#[test]
fn skips_vault_dirs_without_store_age() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    // Half-initialized: dir exists, no store.age.
    std::fs::create_dir_all(dir.join("broken.vault")).unwrap();
    // Only recipients file, still no store.age.
    let half = dir.join("halfbaked.vault");
    std::fs::create_dir_all(&half).unwrap();
    touch(&half.join("recipients"));

    assert!(list_initialized_vaults(dir).is_empty());
}

#[test]
fn skips_non_vault_suffix_dirs() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    // Stray non-.vault subdirs.
    std::fs::create_dir_all(dir.join("notavault")).unwrap();
    std::fs::create_dir_all(dir.join("vault")).unwrap(); // no `.vault` suffix
    // Files (not dirs) with .vault suffix are also ignored.
    touch(&dir.join("something.vault"));

    assert!(list_initialized_vaults(dir).is_empty());
}

#[test]
fn lists_initialized_vaults_sorted() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    for name in &["staging", "default", "production"] {
        let vault = dir.join(format!("{name}.vault"));
        std::fs::create_dir_all(&vault).unwrap();
        touch(&vault.join("store.age"));
    }
    let names = list_initialized_vaults(dir);
    assert_eq!(names, vec!["default", "production", "staging"]);
}

#[test]
fn mixes_initialized_and_not_correctly() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path();
    // 2 initialized
    for name in &["alpha", "gamma"] {
        let vault = dir.join(format!("{name}.vault"));
        std::fs::create_dir_all(&vault).unwrap();
        touch(&vault.join("store.age"));
    }
    // 1 half-initialized
    std::fs::create_dir_all(dir.join("broken.vault")).unwrap();
    // 1 stray
    std::fs::create_dir_all(dir.join("scratch")).unwrap();

    let names = list_initialized_vaults(dir);
    assert_eq!(names, vec!["alpha", "gamma"]);
}
