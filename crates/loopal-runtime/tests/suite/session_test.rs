use loopal_message::Message;
use loopal_runtime::SessionManager;
use std::path::Path;
use tempfile::TempDir;

fn make_manager(tmp: &TempDir) -> SessionManager {
    SessionManager::with_base_dir(tmp.path().to_path_buf())
}

#[test]
fn test_session_manager_new() {
    // SessionManager::new() uses home dir — just verify it does not panic.
    // It may fail if home dir is not available, but that's unusual in CI.
    let result = SessionManager::new();
    assert!(result.is_ok(), "SessionManager::new() should succeed");
}

#[test]
fn test_create_session() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    let cwd = Path::new("/tmp/test_project");
    let session = mgr.create_session(cwd, "test-model").unwrap();

    assert!(!session.id.is_empty(), "session ID should not be empty");
    assert_eq!(session.model, "test-model");
    assert_eq!(session.cwd, cwd.to_string_lossy());
}

#[test]
fn test_resume_session() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    let cwd = Path::new("/tmp/test_project");
    let session = mgr.create_session(cwd, "test-model").unwrap();
    let session_id = session.id.clone();

    // Resume the session
    let (resumed, messages) = mgr.resume_session(&session_id).unwrap();

    assert_eq!(resumed.id, session_id);
    assert_eq!(resumed.model, "test-model");
    assert!(messages.is_empty(), "fresh session should have no messages");
}

#[test]
fn test_save_message_and_resume() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    let cwd = Path::new("/tmp/test_project");
    let session = mgr.create_session(cwd, "test-model").unwrap();
    let session_id = session.id.clone();

    // Save a few messages
    let mut msg1 = Message::user("hello");
    let mut msg2 = Message::assistant("hi there");
    mgr.save_message(&session_id, &mut msg1).unwrap();
    mgr.save_message(&session_id, &mut msg2).unwrap();

    // Resume and verify messages
    let (_resumed, messages) = mgr.resume_session(&session_id).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text_content(), "hello");
    assert_eq!(messages[1].text_content(), "hi there");
}

#[test]
fn test_resume_nonexistent_session_fails() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    let result = mgr.resume_session("nonexistent-session-id-12345");
    assert!(
        result.is_err(),
        "resuming a nonexistent session should fail"
    );
}

#[test]
fn test_list_sessions() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    // Initially empty
    let sessions = mgr.list_sessions().unwrap();
    assert!(sessions.is_empty());

    // Create a session
    let cwd = Path::new("/tmp/test_project");
    let _s1 = mgr.create_session(cwd, "model-a").unwrap();
    let _s2 = mgr.create_session(cwd, "model-b").unwrap();

    let sessions = mgr.list_sessions().unwrap();
    assert_eq!(sessions.len(), 2);
}

#[test]
fn test_update_session() {
    let tmp = TempDir::new().unwrap();
    let mgr = make_manager(&tmp);

    let cwd = Path::new("/tmp/test_project");
    let mut session = mgr.create_session(cwd, "test-model").unwrap();
    let session_id = session.id.clone();

    // Update the title
    session.title = "My Updated Title".to_string();
    mgr.update_session(&session).unwrap();

    // Resume and verify
    let (resumed, _messages) = mgr.resume_session(&session_id).unwrap();
    assert_eq!(resumed.title, "My Updated Title");
}
