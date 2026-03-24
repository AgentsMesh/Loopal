use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_runtime::projection::project_messages;
use loopal_session::ToolCallStatus;
use loopal_tool_api::COMPLETION_PREFIX;

#[test]
fn project_attempt_completion_promoted_to_assistant_message() {
    let assistant_msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "tc-comp".into(),
            name: "AttemptCompletion".into(),
            input: serde_json::json!({"result": "All done"}),
        }],
    };
    let completion_content = format!("{COMPLETION_PREFIX}All done");
    let user_msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tc-comp".into(),
            content: completion_content,
            is_error: false,
        }],
    };
    let display = project_messages(&[assistant_msg, user_msg]);
    // ToolCall should be status "success" but NO result stored
    assert_eq!(display[0].tool_calls[0].name, "AttemptCompletion");
    assert_eq!(display[0].tool_calls[0].status, ToolCallStatus::Success);
    assert!(
        display[0].tool_calls[0].result.is_none(),
        "AttemptCompletion should not store result in tool call"
    );
    // Completion should be promoted to a separate assistant message
    assert_eq!(display.len(), 2);
    assert_eq!(display[1].role, "assistant");
    assert_eq!(display[1].content, "All done");
    assert!(display[1].tool_calls.is_empty());
}

#[test]
fn project_attempt_completion_error_not_promoted() {
    let assistant_msg = Message {
        id: None,
        role: MessageRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "tc-fail".into(),
            name: "AttemptCompletion".into(),
            input: serde_json::json!({"result": "oops"}),
        }],
    };
    let user_msg = Message {
        id: None,
        role: MessageRole::User,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "tc-fail".into(),
            content: "completion failed".into(),
            is_error: true,
        }],
    };
    let display = project_messages(&[assistant_msg, user_msg]);
    // Error should NOT be promoted — treated as normal tool error
    assert_eq!(display.len(), 1);
    assert_eq!(display[0].tool_calls[0].status, ToolCallStatus::Error);
    assert!(display[0].tool_calls[0].result.is_some());
}
