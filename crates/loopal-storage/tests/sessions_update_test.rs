use std::path::Path;

use loopal_storage::SessionStore;
use tempfile::TempDir;

#[test]
fn test_update_session_content_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let mut session = store.create_session(Path::new("/work"), "gpt-4").unwrap();
    session.title = "My Project".to_string();
    session.mode = "plan".to_string();
    store.update_session(&session).unwrap();

    let loaded = store.load_session(&session.id).unwrap();
    assert_eq!(loaded.title, "My Project");
    assert_eq!(loaded.mode, "plan");
    assert_eq!(loaded.model, "gpt-4");
}

#[test]
fn test_load_session_io_error_not_not_found() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let session_dir = tmp.path().join("sessions").join("bad-session");
    std::fs::create_dir_all(&session_dir).unwrap();
    // Create session.json as a directory instead of a file
    std::fs::create_dir_all(session_dir.join("session.json")).unwrap();

    let result = store.load_session("bad-session");
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        !err_msg.contains("Session not found"),
        "should be an IO error, not SessionNotFound, got: {err_msg}",
    );
}
