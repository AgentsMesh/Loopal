use loopal_error::StorageError;
use loopal_storage::SessionStore;

#[test]
fn journal_path_is_owned_by_the_root_session_directory() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::with_base_dir(temp.path().to_path_buf());

    let path = store.workflow_journal_path("session-1", "run-1").unwrap();

    assert_eq!(
        path,
        temp.path().join("sessions/session-1/workflows/run-1.jsonl")
    );
    assert!(!path.exists());
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn journal_path_rejects_non_component_ids() {
    let temp = tempfile::tempdir().unwrap();
    let store = SessionStore::with_base_dir(temp.path().to_path_buf());

    for (session_id, run_id) in [
        ("", "run"),
        ("session", ""),
        (".", "run"),
        ("session", ".."),
        ("../escape", "run"),
        ("session", "nested/run"),
        ("session", "nested\\run"),
        ("/absolute", "run"),
    ] {
        assert!(matches!(
            store.workflow_journal_path(session_id, run_id),
            Err(StorageError::InvalidPathComponent { .. })
        ));
    }
}
