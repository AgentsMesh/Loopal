use loopal_storage::{SessionStore, SubAgentRef};
use loopal_workspace::WorkspaceService;
use loopal_workspace::types::WorkspaceParams;

#[tokio::test]
async fn session_listing_is_cwd_scoped_and_excludes_subagents() {
    let base = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let store = SessionStore::with_base_dir(base.path().to_path_buf());
    let mut parent = store
        .create_session_with_id(root.path(), "model-root", "root-session")
        .unwrap();
    parent.title = "Root session".into();
    parent.mode = "plan".into();
    store.update_session(&parent).unwrap();
    store
        .create_session_with_id(root.path(), "model-sub", "sub-session")
        .unwrap();
    store
        .add_sub_agent(
            "root-session",
            SubAgentRef {
                name: "worker".into(),
                session_id: "sub-session".into(),
                parent: None,
                model: Some("model-sub".into()),
            },
        )
        .unwrap();
    store
        .create_session_with_id(other.path(), "model-other", "other-session")
        .unwrap();
    let service = WorkspaceService::with_session_store(root.path(), store).unwrap();
    let sessions = service
        .list_sessions(WorkspaceParams {
            workspace_id: "local-workspace".into(),
        })
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "root-session");
    assert_eq!(sessions[0].title, "Root session");
    assert_eq!(sessions[0].model, "model-root");
    assert_eq!(sessions[0].mode, "plan");
    assert!(sessions[0].created_at.ends_with('Z'));
    assert!(sessions[0].updated_at.ends_with('Z'));
}

#[tokio::test]
async fn empty_session_title_uses_workspace_basename() {
    let base = tempfile::tempdir().unwrap();
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let store = SessionStore::with_base_dir(base.path().to_path_buf());
    store
        .create_session_with_id(&root, "model-root", "root-session")
        .unwrap();
    let service = WorkspaceService::with_session_store(&root, store).unwrap();
    let sessions = service
        .list_sessions(WorkspaceParams {
            workspace_id: "local-workspace".into(),
        })
        .await
        .unwrap();
    assert_eq!(sessions[0].title, "Loopal session · project");
    assert!(!sessions[0].title.contains("root-session"));
}
