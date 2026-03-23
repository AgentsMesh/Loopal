use loopal_message::Message;
use loopal_runtime::SessionManager;
use tempfile::TempDir;

#[test]
fn clear_history_marker_persisted() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::with_base_dir(tmp.path().to_path_buf());
    let session = mgr
        .create_session(std::path::Path::new("/tmp"), "test-model")
        .unwrap();

    mgr.save_message(&session.id, &mut Message::user("msg1"))
        .unwrap();
    mgr.save_message(&session.id, &mut Message::user("msg2"))
        .unwrap();
    mgr.clear_history(&session.id).unwrap();
    mgr.save_message(&session.id, &mut Message::user("msg3"))
        .unwrap();

    let (_, messages) = mgr.resume_session(&session.id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text_content(), "msg3");
}

#[test]
fn compact_history_marker_persisted() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::with_base_dir(tmp.path().to_path_buf());
    let session = mgr
        .create_session(std::path::Path::new("/tmp"), "test-model")
        .unwrap();

    for i in 0..10 {
        mgr.save_message(&session.id, &mut Message::user(&format!("msg-{i}")))
            .unwrap();
    }
    mgr.compact_history(&session.id, 3).unwrap();

    let (_, messages) = mgr.resume_session(&session.id).unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].text_content(), "msg-7");
    assert_eq!(messages[2].text_content(), "msg-9");
}

#[test]
fn save_message_assigns_uuid() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::with_base_dir(tmp.path().to_path_buf());
    let session = mgr
        .create_session(std::path::Path::new("/tmp"), "test-model")
        .unwrap();

    let mut msg = Message::user("hello");
    assert!(msg.id.is_none());
    mgr.save_message(&session.id, &mut msg).unwrap();

    // In-memory message should now have the UUID
    assert!(msg.id.is_some());
    assert!(!msg.id.as_ref().unwrap().is_empty());

    // Storage should match
    let (_, messages) = mgr.resume_session(&session.id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].id, msg.id);
}

#[test]
fn save_message_preserves_existing_id() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::with_base_dir(tmp.path().to_path_buf());
    let session = mgr
        .create_session(std::path::Path::new("/tmp"), "test-model")
        .unwrap();

    let mut msg = Message::user("hello").with_id("custom-id".into());
    mgr.save_message(&session.id, &mut msg).unwrap();

    let (_, messages) = mgr.resume_session(&session.id).unwrap();
    assert_eq!(messages[0].id.as_deref(), Some("custom-id"));
}
