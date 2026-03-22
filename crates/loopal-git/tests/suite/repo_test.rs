use loopal_git::{current_branch, is_git_repo, repo_root};

use crate::init_repo;

#[test]
fn test_is_git_repo_true() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    assert!(is_git_repo(dir.path()));
}

#[test]
fn test_is_git_repo_false() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!is_git_repo(dir.path()));
}

#[test]
fn test_repo_root_returns_path() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let root = repo_root(dir.path()).expect("should find root");
    // canonicalize both to handle macOS /tmp → /private/tmp
    let expected = dir.path().canonicalize().unwrap();
    let actual = root.canonicalize().unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn test_repo_root_non_repo() {
    let dir = tempfile::tempdir().unwrap();
    assert!(repo_root(dir.path()).is_none());
}

#[test]
fn test_current_branch_returns_name() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let branch = current_branch(dir.path());
    // git init creates "main" or "master" depending on config
    assert!(branch.is_some());
}

#[test]
fn test_current_branch_non_repo() {
    let dir = tempfile::tempdir().unwrap();
    assert!(current_branch(dir.path()).is_none());
}
