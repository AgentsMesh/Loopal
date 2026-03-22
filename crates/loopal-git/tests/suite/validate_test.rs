use loopal_git::{create_worktree, remove_worktree, GitError};

use crate::init_repo;

#[test]
fn test_reject_empty_name() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    assert!(matches!(create_worktree(dir.path(), ""), Err(GitError::InvalidName(_))));
}

#[test]
fn test_reject_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    assert!(matches!(create_worktree(dir.path(), "../../etc"), Err(GitError::InvalidName(_))));
    assert!(matches!(create_worktree(dir.path(), "foo/bar"), Err(GitError::InvalidName(_))));
    assert!(matches!(create_worktree(dir.path(), "foo\\bar"), Err(GitError::InvalidName(_))));
}

#[test]
fn test_reject_dot_prefix() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    assert!(matches!(create_worktree(dir.path(), ".hidden"), Err(GitError::InvalidName(_))));
}

#[test]
fn test_reject_dash_prefix() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    assert!(matches!(create_worktree(dir.path(), "--flag"), Err(GitError::InvalidName(_))));
}

#[test]
fn test_reject_spaces() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    assert!(matches!(create_worktree(dir.path(), "has space"), Err(GitError::InvalidName(_))));
}

#[test]
fn test_remove_also_validates_name() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    assert!(matches!(remove_worktree(dir.path(), "../escape", false), Err(GitError::InvalidName(_))));
}

#[test]
fn test_valid_names_accepted() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    for name in ["simple", "with-dash", "with_underscore", "CamelCase", "v2"] {
        let info = create_worktree(dir.path(), name).unwrap();
        assert!(info.path.exists());
        remove_worktree(dir.path(), name, false).unwrap();
    }
}
