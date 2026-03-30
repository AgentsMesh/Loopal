use loopal_message::Message;
use loopal_runtime::SessionManager;
use loopal_storage::SubAgentRef;
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

#[test]
fn add_sub_agent_and_load_messages() {
    let tmp = TempDir::new().unwrap();
    let mgr = SessionManager::with_base_dir(tmp.path().to_path_buf());

    // Create root session
    let root = mgr
        .create_session(std::path::Path::new("/work"), "root-model")
        .unwrap();

    // Create sub-agent session with messages
    let sub = mgr
        .create_session(std::path::Path::new("/work"), "sub-model")
        .unwrap();
    mgr.save_message(&sub.id, &mut Message::user("do analysis"))
        .unwrap();
    mgr.save_message(&sub.id, &mut Message::assistant("done"))
        .unwrap();

    // Record sub-agent in root session
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

    // On resume, root session has sub-agent refs
    let (resumed, _) = mgr.resume_session(&root.id).unwrap();
    assert_eq!(resumed.sub_agents.len(), 1);
    assert_eq!(resumed.sub_agents[0].name, "researcher");
    assert_eq!(resumed.sub_agents[0].session_id, sub.id);

    // Can load sub-agent messages
    let sub_msgs = mgr.load_messages(&sub.id).unwrap();
    assert_eq!(sub_msgs.len(), 2);
    assert_eq!(sub_msgs[0].text_content(), "do analysis");
    assert_eq!(sub_msgs[1].text_content(), "done");
}
