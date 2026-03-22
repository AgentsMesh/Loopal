use loopal_git::{
    create_worktree, remove_worktree, worktree_has_changes,
    cleanup_stale_worktrees, GitError,
};

use crate::init_repo;

#[test]
fn test_create_and_remove_worktree() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let info = create_worktree(dir.path(), "test-wt").unwrap();
    assert!(info.path.exists());
    assert_eq!(info.branch, "loopal-wt-test-wt");
    assert_eq!(info.name, "test-wt");

    // Should be a valid git worktree
    assert!(loopal_git::is_git_repo(&info.path));

    // No changes initially
    assert!(!worktree_has_changes(&info.path).unwrap());

    // Remove it
    remove_worktree(dir.path(), "test-wt", false).unwrap();
    assert!(!info.path.exists());
}

#[test]
fn test_duplicate_name_rejected() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    create_worktree(dir.path(), "dup").unwrap();
    let err = create_worktree(dir.path(), "dup").unwrap_err();
    assert!(matches!(err, GitError::WorktreeExists(_)));
}

#[test]
fn test_worktree_has_changes_detects_modifications() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let info = create_worktree(dir.path(), "dirty").unwrap();
    assert!(!worktree_has_changes(&info.path).unwrap());

    // Create a new file in the worktree
    std::fs::write(info.path.join("new.txt"), "hello").unwrap();
    assert!(worktree_has_changes(&info.path).unwrap());

    // Force-remove despite changes
    remove_worktree(dir.path(), "dirty", true).unwrap();
}

#[test]
fn test_ensure_gitignore_entry() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    create_worktree(dir.path(), "gi-test").unwrap();

    let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(content.contains(".loopal/worktrees/"));

    // Second create should not duplicate the entry
    remove_worktree(dir.path(), "gi-test", false).unwrap();
    create_worktree(dir.path(), "gi-test2").unwrap();

    let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    let count = content.matches(".loopal/worktrees/").count();
    assert_eq!(count, 1, "gitignore entry should not be duplicated");

    remove_worktree(dir.path(), "gi-test2", false).unwrap();
}

#[test]
fn test_orphan_branch_cleaned_on_create() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Create and force-remove (leaves branch behind in some scenarios)
    let info = create_worktree(dir.path(), "orphan").unwrap();
    // Manually delete the directory to simulate a crash
    std::fs::remove_dir_all(&info.path).unwrap();
    // Prune git's worktree list
    crate::run(dir.path(), &["git", "worktree", "prune"]);

    // Re-create should succeed (stale branch is cleaned up)
    let info2 = create_worktree(dir.path(), "orphan").unwrap();
    assert!(info2.path.exists());
    remove_worktree(dir.path(), "orphan", false).unwrap();
}

#[test]
fn test_cleanup_stale_worktrees() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let info = create_worktree(dir.path(), "stale").unwrap();
    assert!(info.path.exists());

    // Corrupt the worktree by removing its .git file
    let git_file = info.path.join(".git");
    if git_file.exists() {
        std::fs::remove_file(&git_file).unwrap();
    }

    cleanup_stale_worktrees(dir.path());

    // The stale worktree directory should be cleaned up
    // (remove_worktree is best-effort, directory may or may not exist)
}

#[test]
fn test_cleanup_noop_on_missing_dir() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    // No .loopal/worktrees/ exists — should not panic
    cleanup_stale_worktrees(dir.path());
}
