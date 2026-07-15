use std::path::Path;
use std::process::Command;

use loopal_ipc::protocol::methods;
use serde_json::json;

use super::workspace_rpc_support::setup;

#[tokio::test]
async fn git_search_and_worktree_wire_matches_desktop_contract() {
    let root = tempfile::tempdir().unwrap();
    init_repo(root.path());
    let (_hub, conn, _rx) = setup(root.path()).await;
    let before = conn
        .send_request(
            methods::WORKSPACE_READ_FILE.name,
            json!({
                "workspaceId": "local-workspace", "path": "code.rs",
            }),
        )
        .await
        .unwrap();
    conn.send_request(
        methods::WORKSPACE_WRITE_FILE.name,
        json!({
            "workspaceId": "local-workspace", "path": "code.rs",
            "content": "fn after() { /* needle */ }\n", "expectedVersion": before["version"],
        }),
    )
    .await
    .unwrap();
    let search = conn
        .send_request(
            methods::WORKSPACE_SEARCH.name,
            json!({
                "workspaceId": "local-workspace", "query": "needle",
                "glob": "**/*.rs", "maxResults": 20,
            }),
        )
        .await
        .unwrap();
    assert_eq!(search["matches"][0]["path"], "code.rs");
    assert!(search["matches"][0]["column"].as_u64().unwrap() > 0);
    let status = conn
        .send_request(
            methods::WORKSPACE_GIT_STATUS.name,
            json!({
                "workspaceId": "local-workspace",
            }),
        )
        .await
        .unwrap();
    assert_eq!(status["branch"], "main");
    assert_eq!(status["changes"][0]["path"], "code.rs");
    conn.send_request(
        methods::WORKSPACE_GIT_STAGE.name,
        json!({"workspaceId": "local-workspace", "path": "code.rs"}),
    )
    .await
    .unwrap();
    let staged = conn
        .send_request(
            methods::WORKSPACE_GIT_STATUS.name,
            json!({"workspaceId": "local-workspace"}),
        )
        .await
        .unwrap();
    assert_eq!(staged["changes"][0]["indexStatus"], "M");
    conn.send_request(
        methods::WORKSPACE_GIT_UNSTAGE.name,
        json!({"workspaceId": "local-workspace", "path": "code.rs"}),
    )
    .await
    .unwrap();
    let diff = conn
        .send_request(
            methods::WORKSPACE_GIT_DIFF.name,
            json!({
                "workspaceId": "local-workspace", "path": "code.rs",
            }),
        )
        .await
        .unwrap();
    assert!(diff["patch"].as_str().unwrap().contains("needle"));
    let created = conn
        .send_request(
            methods::WORKSPACE_CREATE_WORKTREE.name,
            json!({
                "workspaceId": "local-workspace", "name": "rpc-worktree",
            }),
        )
        .await
        .unwrap();
    assert_eq!(created["id"], "rpc-worktree");
    let listed = conn
        .send_request(
            methods::WORKSPACE_LIST_WORKTREES.name,
            json!({
                "workspaceId": "local-workspace",
            }),
        )
        .await
        .unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 2);
    conn.send_request(
        methods::WORKSPACE_REMOVE_WORKTREE.name,
        json!({
            "workspaceId": "local-workspace", "name": "rpc-worktree", "force": true,
        }),
    )
    .await
    .unwrap();
}

fn init_repo(path: &Path) {
    run_git(path, &["init", "-b", "main"]);
    run_git(path, &["config", "user.email", "desktop@example.invalid"]);
    run_git(path, &["config", "user.name", "Loopal Desktop"]);
    std::fs::write(path.join("code.rs"), "fn before() {}\n").unwrap();
    run_git(path, &["add", "code.rs"]);
    run_git(path, &["commit", "-m", "initial"]);
}

fn run_git(path: &Path, args: &[&str]) {
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
