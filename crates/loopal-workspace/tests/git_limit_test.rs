use std::path::Path;
use std::process::Command;

use loopal_workspace::WorkspaceService;
use loopal_workspace::types::WorkspacePathParams;

#[tokio::test]
async fn oversized_git_diff_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    run(dir.path(), &["init", "-b", "main"]);
    run(
        dir.path(),
        &["config", "user.email", "desktop@example.invalid"],
    );
    run(dir.path(), &["config", "user.name", "Loopal Desktop"]);
    std::fs::write(dir.path().join("large.txt"), "before\n").unwrap();
    run(dir.path(), &["add", "large.txt"]);
    run(dir.path(), &["commit", "-m", "initial"]);
    std::fs::write(dir.path().join("large.txt"), "changed\n".repeat(1_200_000)).unwrap();

    let service = WorkspaceService::new(dir.path()).unwrap();
    let error = service
        .git_diff(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "large.txt".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, "response_too_large");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("large.txt"))
            .unwrap()
            .len(),
        9_600_000
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
