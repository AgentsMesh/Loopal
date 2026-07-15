use std::path::Path;
use std::process::Command;

use loopal_workspace::WorkspaceService;
use loopal_workspace::git_types::{CreateWorktreeParams, RemoveWorktreeParams};
use loopal_workspace::types::{WorkspaceParams, WorkspacePathParams};

#[tokio::test]
async fn git_status_diff_and_worktrees_are_structured() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let service = WorkspaceService::new(dir.path()).unwrap();
    let invalid = service
        .create_worktree(CreateWorktreeParams {
            workspace_id: "local-workspace".into(),
            name: "bad.name".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(invalid.code, "invalid_request");
    std::fs::write(dir.path().join("a.txt"), "after\n").unwrap();
    let status = service
        .git_status(WorkspaceParams {
            workspace_id: "local-workspace".into(),
        })
        .await
        .unwrap();
    assert_eq!(status.branch.as_deref(), Some("main"));
    assert_eq!(status.changes[0].path, "a.txt");
    service
        .git_stage(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "a.txt".into(),
        })
        .await
        .unwrap();
    let staged = service
        .git_status(WorkspaceParams {
            workspace_id: "local-workspace".into(),
        })
        .await
        .unwrap();
    assert_eq!(staged.changes[0].index_status, "M");
    assert_eq!(staged.changes[0].worktree_status, " ");
    service
        .git_unstage(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "a.txt".into(),
        })
        .await
        .unwrap();
    let unstaged = service
        .git_status(WorkspaceParams {
            workspace_id: "local-workspace".into(),
        })
        .await
        .unwrap();
    assert_eq!(unstaged.changes[0].index_status, " ");
    assert_eq!(unstaged.changes[0].worktree_status, "M");
    let diff = service
        .git_diff(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "a.txt".into(),
        })
        .await
        .unwrap();
    assert!(diff.patch.contains("after"));
    assert_eq!(diff.original, "before\n");
    let created = service
        .create_worktree(CreateWorktreeParams {
            workspace_id: "local-workspace".into(),
            name: "desktop-test".into(),
        })
        .await
        .unwrap();
    assert_eq!(created.id, "desktop-test");
    let worktrees = service
        .list_worktrees(WorkspaceParams {
            workspace_id: "local-workspace".into(),
        })
        .await
        .unwrap();
    assert_eq!(worktrees.len(), 2);
    let main = worktrees.iter().find(|worktree| worktree.is_main).unwrap();
    assert_eq!(main.id, "local-workspace");
    service
        .remove_worktree(RemoveWorktreeParams {
            workspace_id: "local-workspace".into(),
            name: "desktop-test".into(),
            force: true,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn worktree_management_rejects_nested_workspace_root() {
    let dir = tempfile::tempdir().unwrap();
    init_repo(dir.path());
    let nested = dir.path().join("nested");
    std::fs::create_dir(&nested).unwrap();
    let service = WorkspaceService::new(&nested).unwrap();
    let error = service
        .list_worktrees(WorkspaceParams {
            workspace_id: "local-workspace".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, "git_root_outside_workspace");
}

fn init_repo(path: &Path) {
    run(path, &["init", "-b", "main"]);
    run(path, &["config", "user.email", "desktop@example.invalid"]);
    run(path, &["config", "user.name", "Loopal Desktop"]);
    std::fs::write(path.join("a.txt"), "before\n").unwrap();
    run(path, &["add", "a.txt"]);
    run(path, &["commit", "-m", "initial"]);
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

#[tokio::test]
async fn unborn_repository_can_diff_stage_and_unstage() {
    let dir = tempfile::tempdir().unwrap();
    run(dir.path(), &["init", "-b", "main"]);
    std::fs::write(dir.path().join("new.txt"), "new line\n").unwrap();
    let service = WorkspaceService::new(dir.path()).unwrap();
    let diff = service
        .git_diff(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "new.txt".into(),
        })
        .await
        .unwrap();
    assert!(diff.patch.contains("/dev/null"));
    service
        .git_stage(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "new.txt".into(),
        })
        .await
        .unwrap();
    let staged = service
        .git_status(WorkspaceParams {
            workspace_id: "local-workspace".into(),
        })
        .await
        .unwrap();
    assert_eq!(staged.changes[0].index_status, "A");
    service
        .git_unstage(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "new.txt".into(),
        })
        .await
        .unwrap();
    let unstaged = service
        .git_status(WorkspaceParams {
            workspace_id: "local-workspace".into(),
        })
        .await
        .unwrap();
    assert_eq!(unstaged.changes[0].index_status, "?");
    assert_eq!(unstaged.changes[0].worktree_status, "?");
}
