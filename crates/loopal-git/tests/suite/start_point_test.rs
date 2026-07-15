use std::process::Command;

use loopal_git::{GitError, create_worktree_at, remove_worktree};

use crate::init_repo;

#[test]
fn creates_worktree_at_the_pinned_commit() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let selected_head = output(dir.path(), &["rev-parse", "HEAD"]);

    std::fs::write(dir.path().join("README.md"), "main moved\n").unwrap();
    crate::run(dir.path(), &["git", "add", "README.md"]);
    crate::run(dir.path(), &["git", "commit", "-m", "move main"]);
    let current_head = output(dir.path(), &["rev-parse", "HEAD"]);
    assert_ne!(selected_head, current_head);

    let info = create_worktree_at(dir.path(), "pinned", &selected_head).unwrap();
    assert_eq!(output(&info.path, &["rev-parse", "HEAD"]), selected_head);
    assert_eq!(output(dir.path(), &["rev-parse", "HEAD"]), current_head);

    remove_worktree(dir.path(), "pinned", false).unwrap();
}

#[test]
fn rejects_non_oid_start_points_before_running_git() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());

    for (index, start_point) in ["HEAD", "deadbeef", "--detach"].iter().enumerate() {
        let name = format!("invalid-{index}");
        let error = create_worktree_at(dir.path(), &name, start_point).unwrap_err();
        assert!(matches!(error, GitError::CommandFailed(message) if message.contains("object ID")));
        assert!(!dir.path().join(".loopal/worktrees").join(&name).exists());
        assert!(
            output(
                dir.path(),
                &["branch", "--list", &format!("loopal-wt-{name}")]
            )
            .is_empty()
        );
    }
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
