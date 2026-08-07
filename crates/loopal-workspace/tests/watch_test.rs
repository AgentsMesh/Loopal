use std::path::Path;
use std::process::Command;
use std::time::Duration;

use loopal_workspace::WorkspaceService;
use loopal_workspace::types::{SearchParams, WorkspaceParams, WorkspacePathParams};
use tokio::sync::broadcast;

#[tokio::test]
async fn readonly_git_status_does_not_feed_watcher() {
    let root = tempfile::tempdir().unwrap();
    init_repo(root.path());
    let service = WorkspaceService::new(root.path()).unwrap();
    let mut events = service.subscribe();

    for _ in 0..5 {
        service
            .git_status(WorkspaceParams {
                workspace_id: "local-workspace".into(),
            })
            .await
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(count_git(&mut events), 0);

    service
        .git_stage(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "tracked.txt".into(),
        })
        .await
        .unwrap();
    assert_eq!(count_git(&mut events), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn search_ignores_symlink_escape_without_a_glob() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("src")).unwrap();
    std::fs::write(root.path().join("src/main.rs"), "fn saved() {}\n").unwrap();
    std::fs::write(outside.path().join("outside.txt"), "saved outside\n").unwrap();
    symlink(
        outside.path().join("outside.txt"),
        root.path().join("escape-link"),
    )
    .unwrap();
    let service = WorkspaceService::new(root.path()).unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        service.search(SearchParams {
            workspace_id: "local-workspace".into(),
            query: "saved".into(),
            glob: None,
            max_results: 200,
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].path, "src/main.rs");
}

fn count_git(events: &mut broadcast::Receiver<loopal_workspace::ServiceNotification>) -> usize {
    let mut count = 0;
    while let Ok(event) = events.try_recv() {
        count += usize::from(event.method == "workspace/gitChanged");
    }
    count
}

fn init_repo(path: &Path) {
    run(path, &["init", "-b", "main"]);
    run(path, &["config", "user.email", "desktop@example.invalid"]);
    run(path, &["config", "user.name", "Loopal Desktop"]);
    std::fs::write(path.join("tracked.txt"), "before\n").unwrap();
    run(path, &["add", "tracked.txt"]);
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
