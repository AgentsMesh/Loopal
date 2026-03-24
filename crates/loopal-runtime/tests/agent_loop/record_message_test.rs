use loopal_message::{ContentBlock, MessageRole};

use super::make_runner;

#[test]
fn test_record_assistant_message_text_only() {
    let (mut runner, _rx) = make_runner();
    assert!(runner.params.store.is_empty());

    runner.record_assistant_message("Hello, world!", &[], "", None, vec![]);

    assert_eq!(runner.params.store.len(), 1);
    let msg = &runner.params.store.messages()[0];
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.content.len(), 1);
    match &msg.content[0] {
        ContentBlock::Text { text } => assert_eq!(text, "Hello, world!"),
        other => panic!("expected Text block, got {other:?}"),
    }
}

#[test]
fn test_record_assistant_message_with_tool_uses() {
    let (mut runner, _rx) = make_runner();

    let tool_uses = vec![
        (
            "tc-1".to_string(),
            "bash".to_string(),
            serde_json::json!({"command": "ls"}),
        ),
        (
            "tc-2".to_string(),
            "read".to_string(),
            serde_json::json!({"file": "test.rs"}),
        ),
    ];

    runner.record_assistant_message("Let me check that.", &tool_uses, "", None, vec![]);

    assert_eq!(runner.params.store.len(), 1);
    let msg = &runner.params.store.messages()[0];
    assert_eq!(msg.role, MessageRole::Assistant);
    assert_eq!(msg.content.len(), 3);

    match &msg.content[0] {
        ContentBlock::Text { text } => assert_eq!(text, "Let me check that."),
        other => panic!("expected Text, got {other:?}"),
    }
    match &msg.content[1] {
        ContentBlock::ToolUse { id, name, .. } => {
            assert_eq!(id, "tc-1");
            assert_eq!(name, "bash");
        }
        other => panic!("expected ToolUse, got {other:?}"),
    }
    match &msg.content[2] {
        ContentBlock::ToolUse { id, name, .. } => {
            assert_eq!(id, "tc-2");
            assert_eq!(name, "read");
        }
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

#[test]
fn test_record_assistant_message_empty_adds_nothing() {
    let (mut runner, _rx) = make_runner();
    runner.record_assistant_message("", &[], "", None, vec![]);

    assert!(
        runner.params.store.is_empty(),
        "empty content should not produce a message"
    );
}

#[test]
fn test_record_assistant_message_tool_uses_only_no_text() {
    let (mut runner, _rx) = make_runner();

    let tool_uses = vec![(
        "tc-1".to_string(),
        "bash".to_string(),
        serde_json::json!({"command": "echo hi"}),
    )];

    runner.record_assistant_message("", &tool_uses, "", None, vec![]);

    assert_eq!(runner.params.store.len(), 1);
    let msg = &runner.params.store.messages()[0];
    assert_eq!(msg.content.len(), 1);
    match &msg.content[0] {
        ContentBlock::ToolUse { id, name, .. } => {
            assert_eq!(id, "tc-1");
            assert_eq!(name, "bash");
        }
        other => panic!("expected ToolUse, got {other:?}"),
    }
}

#[tokio::test]
async fn test_record_assistant_message_saves_to_session() {
    let (mut runner, _rx) = make_runner();
    runner.record_assistant_message("test message", &[], "", None, vec![]);
    assert_eq!(runner.params.store.len(), 1);
    assert_eq!(
        runner.params.store.messages()[0].text_content(),
        "test message"
    );
}
