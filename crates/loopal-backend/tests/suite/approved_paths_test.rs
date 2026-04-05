//! Unit tests for ApprovedPaths (session-scoped approval set).

use std::path::PathBuf;

use loopal_backend::approved::ApprovedPaths;

#[test]
fn empty_set_contains_nothing() {
    let ap = ApprovedPaths::new();
    assert!(!ap.contains(&PathBuf::from("/etc/hosts")));
    assert!(!ap.contains(&PathBuf::from("/tmp/test.txt")));
}

#[test]
fn insert_then_contains() {
    let ap = ApprovedPaths::new();
    let path = PathBuf::from("/etc/nginx/nginx.conf");
    ap.insert(path.clone());
    assert!(ap.contains(&path));
}

#[test]
fn distinct_paths_independent() {
    let ap = ApprovedPaths::new();
    ap.insert(PathBuf::from("/etc/hosts"));
    assert!(ap.contains(&PathBuf::from("/etc/hosts")));
    assert!(!ap.contains(&PathBuf::from("/etc/passwd")));
}

#[test]
fn duplicate_insert_is_idempotent() {
    let ap = ApprovedPaths::new();
    let path = PathBuf::from("/tmp/test.txt");
    ap.insert(path.clone());
    ap.insert(path.clone());
    assert!(ap.contains(&path));
}

#[test]
fn default_is_empty() {
    let ap = ApprovedPaths::default();
    assert!(!ap.contains(&PathBuf::from("/any/path")));
}
