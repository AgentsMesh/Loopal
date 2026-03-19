use loopal_types::message::{Message, MessageRole};
use loopal_types::message_normalize::normalize_messages;

#[test]
fn test_normalize_merges_consecutive_same_role() {
    let messages = vec![
        Message::user("hello"),
        Message::user("world"),
        Message::assistant("hi"),
    ];
    let normalized = normalize_messages(&messages);
    assert_eq!(normalized.len(), 2);
    assert_eq!(normalized[0].role, MessageRole::User);
    assert_eq!(normalized[1].role, MessageRole::Assistant);
}

#[test]
fn test_normalize_preserves_alternating() {
    let messages = vec![
        Message::user("a"),
        Message::assistant("b"),
        Message::user("c"),
    ];
    let normalized = normalize_messages(&messages);
    assert_eq!(normalized.len(), 3);
}

#[test]
fn test_normalize_empty() {
    let normalized = normalize_messages(&[]);
    assert!(normalized.is_empty());
}

#[test]
fn test_normalize_system_messages_filtered_out() {
    let messages = vec![
        Message::system("s1"),
        Message::system("s2"),
        Message::user("u"),
    ];
    let normalized = normalize_messages(&messages);
    assert_eq!(normalized.len(), 1);
    assert_eq!(normalized[0].role, MessageRole::User);
}

#[test]
fn test_normalize_merges_user_after_system_removal() {
    let messages = vec![
        Message::user("a"),
        Message::system("s"),
        Message::user("b"),
    ];
    let normalized = normalize_messages(&messages);
    assert_eq!(normalized.len(), 1);
    assert_eq!(normalized[0].role, MessageRole::User);
    assert_eq!(normalized[0].content.len(), 2);
}

#[test]
fn test_normalize_merges_content_blocks() {
    let messages = vec![Message::user("a"), Message::user("b")];
    let normalized = normalize_messages(&messages);
    assert_eq!(normalized.len(), 1);
    assert_eq!(normalized[0].content.len(), 2);
    assert_eq!(normalized[0].text_content(), "ab");
}

#[test]
fn test_normalize_single_non_system_message() {
    // Covers: L13 false (non-system), L17-18 false (first message, result is empty)
    let messages = vec![Message::user("hello")];
    let normalized = normalize_messages(&messages);
    assert_eq!(normalized.len(), 1);
    assert_eq!(normalized[0].role, MessageRole::User);
    assert_eq!(normalized[0].text_content(), "hello");
}

#[test]
fn test_normalize_interleaved_roles_no_merge() {
    // Covers: L17-18 false path where last.role != msg.role
    let messages = vec![
        Message::user("u1"),
        Message::assistant("a1"),
        Message::user("u2"),
        Message::assistant("a2"),
    ];
    let normalized = normalize_messages(&messages);
    assert_eq!(normalized.len(), 4);
    assert_eq!(normalized[0].role, MessageRole::User);
    assert_eq!(normalized[1].role, MessageRole::Assistant);
    assert_eq!(normalized[2].role, MessageRole::User);
    assert_eq!(normalized[3].role, MessageRole::Assistant);
}

#[test]
fn test_normalize_single_assistant_message() {
    // Single assistant message — tests first-message path for assistant role
    let messages = vec![Message::assistant("only assistant")];
    let normalized = normalize_messages(&messages);
    assert_eq!(normalized.len(), 1);
    assert_eq!(normalized[0].role, MessageRole::Assistant);
}

#[test]
fn test_normalize_only_system_messages_empty_result() {
    // All system messages => empty result
    let messages = vec![Message::system("s1"), Message::system("s2")];
    let normalized = normalize_messages(&messages);
    assert!(normalized.is_empty());
}

#[test]
fn test_normalize_system_between_different_roles() {
    // System message between different roles shouldn't cause merge
    let messages = vec![
        Message::user("a"),
        Message::system("s"),
        Message::assistant("b"),
    ];
    let normalized = normalize_messages(&messages);
    assert_eq!(normalized.len(), 2);
    assert_eq!(normalized[0].role, MessageRole::User);
    assert_eq!(normalized[1].role, MessageRole::Assistant);
}
