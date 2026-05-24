use loopal_runtime::SessionManager;
use loopal_storage::SubAgentRef;
use tempfile::TempDir;

#[test]
fn add_sub_agent_persists_ref() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::with_base_dir(tmp.path().to_path_buf());

    let root = mgr
        .create_session(std::path::Path::new("/work"), "root-model")
        .unwrap();
    let sub = mgr
        .create_session(std::path::Path::new("/work"), "sub-model")
        .unwrap();

    mgr.add_sub_agent(
        &root.id,
        SubAgentRef {
            name: "researcher".into(),
            session_id: sub.id.clone(),
            parent: Some("main".into()),
            model: Some("sub-model".into()),
        },
    )
    .unwrap();

    let (resumed, _turns, _) = mgr.resume_session(&root.id).unwrap();
    assert_eq!(resumed.sub_agents.len(), 1);
    assert_eq!(resumed.sub_agents[0].name, "researcher");
    assert_eq!(resumed.sub_agents[0].session_id, sub.id);
}

// ---------------------------------------------------------------------------
// cwd-based session queries
// ---------------------------------------------------------------------------

#[test]
fn latest_session_for_cwd_finds_most_recent() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::with_base_dir(tmp.path().to_path_buf());

    let _s1 = mgr
        .create_session(std::path::Path::new("/proj"), "m1")
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    let s2 = mgr
        .create_session(std::path::Path::new("/proj"), "m2")
        .unwrap();
    mgr.create_session(std::path::Path::new("/other"), "m3")
        .unwrap();

    let latest = mgr
        .latest_session_for_cwd(std::path::Path::new("/proj"))
        .unwrap()
        .expect("should find a session");
    assert_eq!(latest.id, s2.id);
}

#[test]
fn latest_session_for_cwd_returns_none_for_unknown_dir() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::with_base_dir(tmp.path().to_path_buf());

    mgr.create_session(std::path::Path::new("/a"), "m1")
        .unwrap();

    let result = mgr
        .latest_session_for_cwd(std::path::Path::new("/unknown"))
        .unwrap();
    assert!(result.is_none());
}

#[test]
fn list_sessions_for_cwd_filters_correctly() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::with_base_dir(tmp.path().to_path_buf());

    mgr.create_session(std::path::Path::new("/alpha"), "m1")
        .unwrap();
    mgr.create_session(std::path::Path::new("/alpha"), "m2")
        .unwrap();
    mgr.create_session(std::path::Path::new("/beta"), "m3")
        .unwrap();

    let alpha = mgr
        .list_sessions_for_cwd(std::path::Path::new("/alpha"))
        .unwrap();
    assert_eq!(alpha.len(), 2);

    let beta = mgr
        .list_sessions_for_cwd(std::path::Path::new("/beta"))
        .unwrap();
    assert_eq!(beta.len(), 1);

    let empty = mgr
        .list_sessions_for_cwd(std::path::Path::new("/gamma"))
        .unwrap();
    assert!(empty.is_empty());
}
