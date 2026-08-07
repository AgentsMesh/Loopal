use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_ipc::rpc_error::RpcError;
use serde_json::json;

use super::workspace_rpc_support::setup;

#[tokio::test]
async fn ui_acl_rejects_privileged_hub_methods() {
    let root = tempfile::tempdir().unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;
    for method in ["hub/secret/get", "hub/spawn_agent", "hub/mcp/list_tools"] {
        let error = conn.send_request(method, json!({})).await.unwrap_err();
        assert!(
            matches!(error, RpcError::Remote { .. }),
            "{method}: {error:?}"
        );
    }
}

#[tokio::test]
async fn workspace_rpc_emits_notifications() {
    let root = tempfile::tempdir().unwrap();
    let (_hub, conn, mut rx) = setup(root.path()).await;
    let document = conn
        .send_request(
            methods::WORKSPACE_WRITE_FILE.name,
            json!({
                "workspaceId": "local-workspace", "path": "hello.txt",
                "content": "hello workspace", "expectedVersion": null,
            }),
        )
        .await
        .unwrap();
    assert_eq!(document["languageId"], "plaintext");
    let listing = conn
        .send_request(
            methods::WORKSPACE_LIST_DIRECTORY.name,
            json!({
                "workspaceId": "local-workspace", "path": "",
            }),
        )
        .await
        .unwrap();
    assert_eq!(listing["entries"][0]["path"], "hello.txt");
    let mut file_changed = false;
    while !file_changed {
        let message = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv())
            .await
            .unwrap()
            .unwrap();
        if let Incoming::Notification { method, params } = message
            && method == methods::WORKSPACE_FILE_CHANGED.name
            && params["path"] == "hello.txt"
        {
            file_changed = true;
        }
    }
}

#[tokio::test]
async fn removed_raw_pty_rpc_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let (_hub, conn, _rx) = setup(root.path()).await;
    let error = conn
        .send_request("terminal/create", json!({}))
        .await
        .unwrap_err();
    assert!(matches!(error, RpcError::Remote { .. }));
}

#[tokio::test]
async fn non_ui_dispatch_cannot_reach_workspace() {
    let root = tempfile::tempdir().unwrap();
    let (hub, _conn, _rx) = setup(root.path()).await;
    let result = loopal_agent_hub::dispatch::dispatch_hub_request(
        &hub,
        methods::WORKSPACE_LIST_DIRECTORY.name,
        json!({"workspaceId": "local-workspace", "path": ""}),
        "agent-worker".into(),
    )
    .await;
    assert!(result.unwrap_err().contains("require a UI client"));
}
