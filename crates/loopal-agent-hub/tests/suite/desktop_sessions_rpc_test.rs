use loopal_ipc::protocol::methods;
use loopal_storage::SessionStore;
use loopal_workspace::WorkspaceService;
use serde_json::json;

use super::workspace_rpc_support::setup;

#[tokio::test]
async fn desktop_session_rpc_lists_only_workspace_roots() {
    let base = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let store = SessionStore::with_base_dir(base.path().to_path_buf());
    store
        .create_session_with_id(root.path(), "root-model", "root-session")
        .unwrap();
    store
        .create_session_with_id(outside.path(), "outside-model", "outside-session")
        .unwrap();
    let service = WorkspaceService::with_session_store(root.path(), store).unwrap();
    let (hub, conn, _rx) = setup(root.path()).await;
    hub.lock().await.workspace = Some(service);
    let sessions = conn
        .send_request(
            methods::DESKTOP_LIST_SESSIONS.name,
            json!({"workspaceId": "local-workspace"}),
        )
        .await
        .unwrap();
    let sessions = sessions.as_array().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], "root-session");
    assert_eq!(sessions[0]["model"], "root-model");
    assert!(sessions[0]["createdAt"].as_str().unwrap().ends_with('Z'));
}
