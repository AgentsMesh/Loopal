use std::path::Path;
use std::process::Command;

use crate::{inspect_working_directory, prepare_worktree_directory};

#[test]
fn removes_a_clean_worktree_when_the_selected_subdirectory_is_untracked() {
    let root = repository();
    let selected_path = root.path().join("untracked");
    std::fs::create_dir(&selected_path).unwrap();
    let selected = inspect_working_directory(&selected_path).unwrap();
    let git = selected.git.unwrap();

    let error = prepare_worktree_directory(
        Path::new(&selected.path),
        "missing",
        Path::new(&git.root),
        git.head.as_deref(),
    )
    .unwrap_err();

    assert_eq!(error.code, "worktree_directory_missing");
    assert!(!root.path().join(".loopal/worktrees/missing").exists());
    assert!(output(root.path(), &["branch", "--list", "loopal-wt-missing"]).is_empty());
}

#[cfg(unix)]
#[test]
fn retains_hook_changes_when_post_create_validation_fails() {
    use std::os::unix::fs::PermissionsExt;

    let root = repository();
    let selected_path = root.path().join("untracked");
    std::fs::create_dir(&selected_path).unwrap();
    let hook = root.path().join(".git/hooks/post-checkout");
    std::fs::write(
        &hook,
        "#!/bin/sh\nprintf 'retain\\n' > retained-by-hook.txt\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&hook).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&hook, permissions).unwrap();
    let selected = inspect_working_directory(&selected_path).unwrap();
    let git = selected.git.unwrap();
    let expected_head = git.head.unwrap();

    let error = prepare_worktree_directory(
        Path::new(&selected.path),
        "retained",
        Path::new(&git.root),
        Some(&expected_head),
    )
    .unwrap_err();

    let worktree = root.path().join(".loopal/worktrees/retained");
    assert_eq!(error.code, "worktree_retained");
    assert!(error.message.contains(worktree.to_string_lossy().as_ref()));
    assert!(error.message.contains("loopal-wt-retained"));
    assert_eq!(
        std::fs::read_to_string(worktree.join("retained-by-hook.txt")).unwrap(),
        "retain\n"
    );
    assert_eq!(
        output(root.path(), &["rev-parse", "loopal-wt-retained"]),
        expected_head
    );
}

fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    run(root.path(), &["init", "-b", "main"]);
    run(
        root.path(),
        &["config", "user.email", "desktop@example.invalid"],
    );
    run(root.path(), &["config", "user.name", "Loopal Desktop"]);
    std::fs::write(root.path().join("tracked.txt"), "tracked\n").unwrap();
    run(root.path(), &["add", "."]);
    run(root.path(), &["commit", "-m", "initial"]);
    root
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

fn output(path: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}
