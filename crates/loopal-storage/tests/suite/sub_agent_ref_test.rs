use std::path::Path;

use loopal_storage::{SessionStore, SubAgentRef};
use tempfile::TempDir;

#[test]
fn add_sub_agent_persists_to_session() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let session = store.create_session(Path::new("/work"), "model").unwrap();
    assert!(session.sub_agents.is_empty());

    store
        .add_sub_agent(
            &session.id,
            SubAgentRef {
                name: "worker".into(),
                session_id: "sub-123".into(),
                parent: Some("main".into()),
                model: Some("sonnet".into()),
            },
        )
        .unwrap();

    let loaded = store.load_session(&session.id).unwrap();
    assert_eq!(loaded.sub_agents.len(), 1);
    assert_eq!(loaded.sub_agents[0].name, "worker");
    assert_eq!(loaded.sub_agents[0].session_id, "sub-123");
    assert_eq!(loaded.sub_agents[0].parent.as_deref(), Some("main"));
    assert_eq!(loaded.sub_agents[0].model.as_deref(), Some("sonnet"));
}

#[test]
fn add_sub_agent_deduplicates_by_session_id() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let session = store.create_session(Path::new("/work"), "model").unwrap();

    let sub_ref = SubAgentRef {
        name: "worker".into(),
        session_id: "sub-456".into(),
        parent: None,
        model: None,
    };

    store.add_sub_agent(&session.id, sub_ref.clone()).unwrap();
    store.add_sub_agent(&session.id, sub_ref).unwrap();

    let loaded = store.load_session(&session.id).unwrap();
    assert_eq!(
        loaded.sub_agents.len(),
        1,
        "duplicate session_id should be ignored"
    );
}

#[test]
fn add_sub_agent_multiple_agents() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let session = store.create_session(Path::new("/work"), "model").unwrap();

    store
        .add_sub_agent(
            &session.id,
            SubAgentRef {
                name: "a".into(),
                session_id: "s-a".into(),
                parent: None,
                model: None,
            },
        )
        .unwrap();
    store
        .add_sub_agent(
            &session.id,
            SubAgentRef {
                name: "b".into(),
                session_id: "s-b".into(),
                parent: Some("a".into()),
                model: Some("opus".into()),
            },
        )
        .unwrap();

    let loaded = store.load_session(&session.id).unwrap();
    assert_eq!(loaded.sub_agents.len(), 2);
    assert_eq!(loaded.sub_agents[0].name, "a");
    assert_eq!(loaded.sub_agents[1].name, "b");
}

#[test]
fn legacy_session_without_sub_agents_loads_fine() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    // Simulate a legacy session.json without sub_agents field
    let session_dir = tmp.path().join("sessions").join("legacy-id");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("session.json"),
        r#"{
            "id": "legacy-id",
            "title": "Old Session",
            "model": "claude-3",
            "cwd": "/old",
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-01T00:00:00Z",
            "mode": "default"
        }"#,
    )
    .unwrap();

    let loaded = store.load_session("legacy-id").unwrap();
    assert!(
        loaded.sub_agents.is_empty(),
        "legacy sessions should default to empty sub_agents"
    );
}
