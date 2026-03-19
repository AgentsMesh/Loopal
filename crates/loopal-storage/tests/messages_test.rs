use loopal_storage::MessageStore;
use loopal_types::message::Message;
use tempfile::TempDir;

#[test]
fn test_append_and_load_messages() {
    let tmp = TempDir::new().unwrap();
    let store = MessageStore::with_base_dir(tmp.path().to_path_buf());
    let session_id = "test-session";

    // Create session directory
    std::fs::create_dir_all(tmp.path().join("sessions").join(session_id)).unwrap();

    store
        .append_message(session_id, &Message::user("hello"))
        .unwrap();
    store
        .append_message(session_id, &Message::assistant("hi there"))
        .unwrap();

    let messages = store.load_messages(session_id).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text_content(), "hello");
    assert_eq!(messages[1].text_content(), "hi there");
}

#[test]
fn test_load_messages_empty() {
    let tmp = TempDir::new().unwrap();
    let store = MessageStore::with_base_dir(tmp.path().to_path_buf());

    let messages = store.load_messages("nonexistent").unwrap();
    assert!(messages.is_empty());
}

#[test]
fn test_append_creates_directory() {
    let tmp = TempDir::new().unwrap();
    let store = MessageStore::with_base_dir(tmp.path().to_path_buf());

    store
        .append_message("new-session", &Message::user("test"))
        .unwrap();

    let messages = store.load_messages("new-session").unwrap();
    assert_eq!(messages.len(), 1);
}

#[test]
fn test_load_messages_preserves_roles() {
    let tmp = TempDir::new().unwrap();
    let store = MessageStore::with_base_dir(tmp.path().to_path_buf());
    let session_id = "role-test-session";

    store.append_message(session_id, &Message::user("question")).unwrap();
    store.append_message(session_id, &Message::assistant("answer")).unwrap();
    store.append_message(session_id, &Message::system("notice")).unwrap();

    let messages = store.load_messages(session_id).unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].role, loopal_types::message::MessageRole::User);
    assert_eq!(messages[1].role, loopal_types::message::MessageRole::Assistant);
    assert_eq!(messages[2].role, loopal_types::message::MessageRole::System);
}

#[test]
fn test_append_multiple_messages_incrementally() {
    let tmp = TempDir::new().unwrap();
    let store = MessageStore::with_base_dir(tmp.path().to_path_buf());
    let session_id = "incremental-session";

    for i in 0..10 {
        store
            .append_message(session_id, &Message::user(&format!("msg-{}", i)))
            .unwrap();
    }

    let messages = store.load_messages(session_id).unwrap();
    assert_eq!(messages.len(), 10);
    for (i, msg) in messages.iter().enumerate() {
        assert_eq!(msg.text_content(), format!("msg-{i}"));
    }
}

#[test]
fn test_load_messages_handles_empty_lines() {
    let tmp = TempDir::new().unwrap();
    let session_id = "empty-lines-session";

    // Create the messages file with some empty lines mixed in
    let session_dir = tmp.path().join("sessions").join(session_id);
    std::fs::create_dir_all(&session_dir).unwrap();

    let msg = Message::user("hello");
    let line = serde_json::to_string(&msg).unwrap();
    let content = format!("{}\n\n{}\n\n", line, line);
    std::fs::write(session_dir.join("messages.jsonl"), content).unwrap();

    let store = MessageStore::with_base_dir(tmp.path().to_path_buf());
    let messages = store.load_messages(session_id).unwrap();
    assert_eq!(messages.len(), 2, "empty lines should be skipped");
}

#[test]
fn test_load_messages_invalid_jsonl_returns_error() {
    let tmp = TempDir::new().unwrap();
    let session_id = "bad-jsonl-session";

    let session_dir = tmp.path().join("sessions").join(session_id);
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("messages.jsonl"),
        "this is not valid json\n",
    )
    .unwrap();

    let store = MessageStore::with_base_dir(tmp.path().to_path_buf());
    let result = store.load_messages(session_id);
    assert!(result.is_err(), "invalid JSONL should produce an error");
}
