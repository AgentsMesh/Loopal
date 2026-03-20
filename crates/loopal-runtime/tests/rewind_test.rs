use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_runtime::agent_loop::rewind::{detect_turn_boundaries, turn_preview};

#[test]
fn empty_messages_no_boundaries() {
    assert!(detect_turn_boundaries(&[]).is_empty());
}

#[test]
fn single_user_turn() {
    let messages = vec![Message::user("hello")];
    assert_eq!(detect_turn_boundaries(&messages), vec![0]);
}

#[test]
fn multiple_turns_with_assistant() {
    let messages = vec![
        Message::user("q1"),
        Message::assistant("a1"),
        Message::user("q2"),
        Message::assistant("a2"),
    ];
    assert_eq!(detect_turn_boundaries(&messages), vec![0, 2]);
}

#[test]
fn tool_result_only_user_msg_not_a_turn() {
    let tool_result_msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "t1".into(),
            content: "result".into(),
            is_error: false,
        }],
    };
    let messages = vec![
        Message::user("turn1"),
        Message::assistant("response"),
        tool_result_msg,
        Message::user("turn2"),
    ];
    let boundaries = detect_turn_boundaries(&messages);
    assert_eq!(boundaries, vec![0, 3]);
}

#[test]
fn turn_preview_truncates() {
    let msg = Message::user("a]short");
    assert_eq!(turn_preview(&msg, 60), "a]short");

    let long = "x".repeat(100);
    let msg2 = Message::user(&long);
    let preview = turn_preview(&msg2, 60);
    assert!(preview.ends_with("..."));
    // 60 chars + "..."
    assert_eq!(preview.chars().count(), 63);
}

#[test]
fn assistant_only_messages_no_turns() {
    let messages = vec![
        Message::assistant("system init"),
        Message::assistant("more"),
    ];
    assert!(detect_turn_boundaries(&messages).is_empty());
}
