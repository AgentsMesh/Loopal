use std::process::Command;

use loopal_git::{GitError, cleanup_if_clean, create_worktree, remove_worktree};

use crate::init_repo;

#[test]
fn default_removal_preserves_clean_worktree_with_unique_commits() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let info = create_worktree(dir.path(), "valuable").unwrap();
    std::fs::write(info.path.join("result.txt"), "keep me\n").unwrap();
    crate::run(&info.path, &["git", "add", "result.txt"]);
    crate::run(&info.path, &["git", "commit", "-m", "valuable result"]);
    let commit = output(&info.path, &["rev-parse", "HEAD"]);
    assert!(output(&info.path, &["status", "--porcelain"]).is_empty());

    assert!(!cleanup_if_clean(dir.path(), &info));
    let error = remove_worktree(dir.path(), "valuable", false).unwrap_err();
    assert!(matches!(error, GitError::CommandFailed(message) if message.contains("not merged")));
    assert!(info.path.exists());
    assert_eq!(
        output(dir.path(), &["rev-parse", "refs/heads/loopal-wt-valuable"]),
        commit,
    );

    remove_worktree(dir.path(), "valuable", true).unwrap();
    assert!(!info.path.exists());
    assert!(branch(dir.path(), "loopal-wt-valuable").is_empty());
}

#[test]
fn default_cleanup_removes_a_clean_fully_merged_worktree() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let info = create_worktree(dir.path(), "merged").unwrap();
    std::fs::write(info.path.join("merged.txt"), "merged\n").unwrap();
    crate::run(&info.path, &["git", "add", "merged.txt"]);
    crate::run(&info.path, &["git", "commit", "-m", "merged result"]);
    crate::run(
        dir.path(),
        &["git", "merge", "--ff-only", "loopal-wt-merged"],
    );

    assert!(cleanup_if_clean(dir.path(), &info));
    assert!(!info.path.exists());
    assert!(branch(dir.path(), "loopal-wt-merged").is_empty());
}

fn branch(path: &std::path::Path, name: &str) -> String {
    output(path, &["branch", "--list", name])
}

fn output(path: &std::path::Path, args: &[&str]) -> String {
    let result = Command::new("git")
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
