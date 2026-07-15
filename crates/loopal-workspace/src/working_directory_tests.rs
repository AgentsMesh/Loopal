use std::path::Path;
use std::process::Command;

use crate::{inspect_working_directory, prepare_worktree_directory};

#[test]
fn inspects_git_and_prepares_the_selected_subdirectory() {
    let root = tempfile::tempdir().unwrap();
    run(root.path(), &["init", "-b", "main"]);
    run(
        root.path(),
        &["config", "user.email", "desktop@example.invalid"],
    );
    run(root.path(), &["config", "user.name", "Loopal Desktop"]);
    std::fs::create_dir(root.path().join("nested")).unwrap();
    std::fs::write(root.path().join("nested/file.txt"), "tracked\n").unwrap();
    run(root.path(), &["add", "."]);
    run(root.path(), &["commit", "-m", "initial"]);

    let selected = inspect_working_directory(&root.path().join("nested")).unwrap();
    let git = selected.git.unwrap();
    assert_eq!(git.branch.as_deref(), Some("main"));
    assert!(!git.dirty);
    assert!(git.head.is_some());
    std::fs::write(root.path().join("second.txt"), "second\n").unwrap();
    run(root.path(), &["add", "."]);
    run(root.path(), &["commit", "-m", "second"]);
    assert_eq!(
        prepare_worktree_directory(
            Path::new(&selected.path),
            "stale",
            Path::new(&git.root),
            git.head.as_deref(),
        )
        .unwrap_err()
        .code,
        "working_directory_changed"
    );
    let git = inspect_working_directory(&root.path().join("nested"))
        .unwrap()
        .git
        .unwrap();
    let worktree = prepare_worktree_directory(
        Path::new(&selected.path),
        "desktop-1",
        Path::new(&git.root),
        git.head.as_deref(),
    )
    .unwrap();
    assert_eq!(worktree.branch, "loopal-wt-desktop-1");
    assert_eq!(
        worktree.path,
        Path::new(&worktree.path)
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
    );
    assert!(Path::new(&worktree.path).ends_with("desktop-1/nested"));
    assert!(Path::new(&worktree.path).join("file.txt").is_file());
}

#[test]
fn rejects_unsafe_names_and_non_git_worktree_requests() {
    let root = tempfile::tempdir().unwrap();
    assert_eq!(
        prepare_worktree_directory(root.path(), "../escape", root.path(), None,)
            .unwrap_err()
            .code,
        "invalid_worktree_name"
    );
    let canonical = root.path().canonicalize().unwrap();
    assert_eq!(
        prepare_worktree_directory(&canonical, "valid-name", root.path(), None,)
            .unwrap_err()
            .code,
        "not_git_repository"
    );
}

#[test]
fn rejects_worktrees_in_an_unborn_repository() {
    let root = tempfile::tempdir().unwrap();
    run(root.path(), &["init", "-b", "main"]);
    let selected = inspect_working_directory(root.path()).unwrap();
    let git = selected.git.unwrap();
    assert!(git.head.is_none());
    assert_eq!(
        prepare_worktree_directory(
            Path::new(&selected.path),
            "unborn",
            Path::new(&git.root),
            None,
        )
        .unwrap_err()
        .code,
        "worktree_unborn_head"
    );
}

fn run(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
