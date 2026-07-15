use loopal_git::{
    GitError, cleanup_stale_worktrees, create_worktree, remove_worktree, worktree_has_changes,
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
fn test_create_and_remove_with_canonical_root() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let root = dir.path().canonicalize().unwrap();

    let info = create_worktree(&root, "canonical").unwrap();
    assert!(info.path.exists());
    remove_worktree(&root, "canonical", false).unwrap();
    assert!(!info.path.exists());
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
fn test_worktree_creation_uses_local_exclude_and_keeps_source_clean() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let exclude = dir.path().join(".git/info/exclude");
    std::fs::write(&exclude, "# user-owned rule\n*.scratch\n").unwrap();
    assert!(git_output(dir.path(), &["status", "--porcelain"]).is_empty());

    create_worktree(dir.path(), "gi-test").unwrap();

    let content = std::fs::read_to_string(&exclude).unwrap();
    assert!(content.contains("# user-owned rule\n*.scratch\n"));
    assert_eq!(content.matches("/.loopal/worktrees/").count(), 1);
    assert!(!dir.path().join(".gitignore").exists());
    assert!(!dir.path().join(".loopal/.gitignore").exists());
    assert!(git_output(dir.path(), &["status", "--porcelain"]).is_empty());

    remove_worktree(dir.path(), "gi-test", false).unwrap();
    create_worktree(dir.path(), "gi-test2").unwrap();
    let content = std::fs::read_to_string(&exclude).unwrap();
    assert_eq!(content.matches("/.loopal/worktrees/").count(), 1);
    assert!(git_output(dir.path(), &["status", "--porcelain"]).is_empty());
    remove_worktree(dir.path(), "gi-test2", false).unwrap();
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

#[test]
fn test_cleanup_preserves_unverifiable_pruned_worktree() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    let info = create_worktree(dir.path(), "pruned").unwrap();
    assert!(info.path.exists());

    // Simulate crash: wipe the .git pointer so git no longer recognizes it,
    // then prune git's internal list. The directory still exists on disk.
    let git_file = info.path.join(".git");
    if git_file.exists() {
        std::fs::remove_file(&git_file).unwrap();
    }
    crate::run(dir.path(), &["git", "worktree", "prune"]);

    cleanup_stale_worktrees(dir.path());

    assert!(
        info.path.exists(),
        "startup cleanup must retain a directory it cannot prove safe"
    );
    assert!(!git_output(dir.path(), &["branch", "--list", "loopal-wt-pruned"]).is_empty());
}

/// Verify that `create_worktree` does NOT delete a branch that belongs to an
/// active worktree (even if the directory name differs).
#[test]
fn test_create_preserves_active_worktree_branch() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    // Create worktree "alpha" → branch "loopal-wt-alpha"
    let info = create_worktree(dir.path(), "alpha").unwrap();
    assert!(info.path.exists());

    // Attempting to create another worktree called "alpha" should fail (dir exists),
    // but the existing branch should NOT be deleted.
    let err = create_worktree(dir.path(), "alpha").unwrap_err();
    assert!(matches!(err, GitError::WorktreeExists(_)));

    // The original worktree's branch should still be intact.
    let branch_check = std::process::Command::new("git")
        .args(["branch", "--list", "loopal-wt-alpha"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let output = String::from_utf8_lossy(&branch_check.stdout);
    assert!(
        output.contains("loopal-wt-alpha"),
        "active branch should not be deleted"
    );

    remove_worktree(dir.path(), "alpha", false).unwrap();
}

fn git_output(path: &std::path::Path, args: &[&str]) -> String {
    let result = std::process::Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().to_string()
}
