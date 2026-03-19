use std::path::Path;

use loopal_storage::SessionStore;
use tempfile::TempDir;

#[test]
fn test_create_and_load_session() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let session = store
        .create_session(Path::new("/tmp/project"), "test-model")
        .unwrap();
    assert_eq!(session.model, "test-model");
    assert!(!session.id.is_empty());

    let loaded = store.load_session(&session.id).unwrap();
    assert_eq!(loaded.id, session.id);
    assert_eq!(loaded.model, "test-model");
}

#[test]
fn test_update_session() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let mut session = store.create_session(Path::new("/tmp"), "model").unwrap();
    session.title = "Updated Title".to_string();
    store.update_session(&session).unwrap();

    let loaded = store.load_session(&session.id).unwrap();
    assert_eq!(loaded.title, "Updated Title");
}

#[test]
fn test_list_sessions() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    store.create_session(Path::new("/a"), "m1").unwrap();
    store.create_session(Path::new("/b"), "m2").unwrap();

    let sessions = store.list_sessions().unwrap();
    assert_eq!(sessions.len(), 2);
}

#[test]
fn test_load_nonexistent_session() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let result = store.load_session("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_list_sessions_empty() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let sessions = store.list_sessions().unwrap();
    assert!(sessions.is_empty());
}

#[test]
fn test_load_nonexistent_session_error_message() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let result = store.load_session("does-not-exist-xyz");
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("does-not-exist-xyz") || err_msg.contains("not found") || err_msg.contains("NotFound"),
        "error should reference the session id, got: {}",
        err_msg
    );
}

#[test]
fn test_update_nonexistent_session_returns_error() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let session = loopal_storage::Session {
        id: "nonexistent-session-id".to_string(),
        title: "Ghost".to_string(),
        model: "model".to_string(),
        cwd: "/tmp".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        mode: "default".to_string(),
    };

    let result = store.update_session(&session);
    assert!(result.is_err(), "updating a nonexistent session should fail");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("nonexistent-session-id") || err_msg.contains("not found"),
        "error should mention session id, got: {}",
        err_msg
    );
}

#[test]
fn test_list_sessions_sorted_newest_first() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let s1 = store.create_session(Path::new("/a"), "m1").unwrap();
    // Small delay to ensure different timestamps
    std::thread::sleep(std::time::Duration::from_millis(10));
    let s2 = store.create_session(Path::new("/b"), "m2").unwrap();

    let sessions = store.list_sessions().unwrap();
    assert_eq!(sessions.len(), 2);
    // Newest first
    assert_eq!(sessions[0].id, s2.id);
    assert_eq!(sessions[1].id, s1.id);
}

#[test]
fn test_create_session_fields() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    let session = store.create_session(Path::new("/home/user/project"), "claude-3").unwrap();
    assert!(!session.id.is_empty());
    assert_eq!(session.model, "claude-3");
    assert_eq!(session.cwd, "/home/user/project");
    assert!(session.title.is_empty(), "new session title should be empty");
    assert_eq!(session.mode, "default");
    assert!(session.created_at <= session.updated_at);
}

#[test]
fn test_list_sessions_ignores_corrupt_files() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    // Create a valid session
    let _valid = store.create_session(Path::new("/valid"), "m1").unwrap();

    // Create a corrupt session directory
    let corrupt_dir = tmp.path().join("sessions").join("corrupt-session");
    std::fs::create_dir_all(&corrupt_dir).unwrap();
    std::fs::write(corrupt_dir.join("session.json"), "{ not valid json ]]").unwrap();

    let sessions = store.list_sessions().unwrap();
    // Only the valid session should appear; corrupt one should be silently skipped
    assert_eq!(sessions.len(), 1);
}

#[test]
fn test_list_sessions_ignores_dirs_without_session_json() {
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    // Create a directory without session.json
    let orphan_dir = tmp.path().join("sessions").join("orphan");
    std::fs::create_dir_all(&orphan_dir).unwrap();
    std::fs::write(orphan_dir.join("other.txt"), "not a session").unwrap();

    let sessions = store.list_sessions().unwrap();
    assert!(sessions.is_empty(), "directory without session.json should be ignored");
}

#[test]
fn test_list_sessions_ignores_files_in_sessions_dir() {
    // L114: entry.file_type().is_dir() — false path (a file, not dir)
    let tmp = TempDir::new().unwrap();
    let store = SessionStore::with_base_dir(tmp.path().to_path_buf());

    // Create sessions dir with a plain file (not a directory)
    let sessions_dir = tmp.path().join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();
    std::fs::write(sessions_dir.join("stray-file.txt"), "not a session dir").unwrap();

    let sessions = store.list_sessions().unwrap();
    assert!(sessions.is_empty(), "plain files should be ignored");
}

