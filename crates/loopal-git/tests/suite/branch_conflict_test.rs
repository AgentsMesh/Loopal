use std::process::Command;

use loopal_git::{GitError, create_worktree};

use crate::init_repo;

#[test]
fn create_preserves_existing_branch_and_unique_commit() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let original = output(dir.path(), &["branch", "--show-current"]);

    crate::run(dir.path(), &["git", "switch", "-c", "loopal-wt-preserve"]);
    std::fs::write(dir.path().join("unique.txt"), "must survive\n").unwrap();
    crate::run(dir.path(), &["git", "add", "unique.txt"]);
    crate::run(
        dir.path(),
        &["git", "commit", "-m", "unique worktree commit"],
    );
    let unique_commit = output(dir.path(), &["rev-parse", "HEAD"]);
    crate::run(dir.path(), &["git", "switch", &original]);

    let error = create_worktree(dir.path(), "preserve").unwrap_err();
    assert!(matches!(error, GitError::WorktreeExists(ref name) if name == "preserve"));
    assert!(!dir.path().join(".loopal/worktrees/preserve").exists());
    assert_eq!(
        output(dir.path(), &["rev-parse", "refs/heads/loopal-wt-preserve"]),
        unique_commit,
    );
    crate::run(
        dir.path(),
        &[
            "git",
            "cat-file",
            "-e",
            &format!("{unique_commit}^{{commit}}"),
        ],
    );
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
