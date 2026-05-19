use loopal_message::{ContentBlock, Message, MessageRole};
use loopal_protocol::projection::project_messages;
use loopal_tool_invocation::{CancelCause, StaleReason, ToolResultMetadata};

#[test]
fn project_preserves_tool_result_metadata() {
    let msgs = vec![
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "tc-meta".into(),
                name: "Bash".into(),
                input: serde_json::json!({"command": "ls"}),
            }],
            origin: None,
        },
        Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "tc-meta".into(),
                content: "Watchdog timeout".into(),
                is_error: true,
                metadata: Some(ToolResultMetadata::stale(StaleReason::WatchdogTimeout)),
            }],
            origin: None,
        },
    ];
    let display = project_messages(&msgs);
    let tc = &display[0].tool_calls[0];
    let metadata = tc
        .metadata
        .as_ref()
        .expect("metadata must flow through projection");
    match metadata {
        ToolResultMetadata::Stale { reason } => assert_eq!(*reason, StaleReason::WatchdogTimeout),
        other => panic!("expected Stale, got {other:?}"),
    }
}

#[test]
fn project_preserves_cancel_cause_metadata() {
    let msgs = vec![
        Message {
            id: None,
            role: MessageRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "tc-cancel".into(),
                name: "Bash".into(),
                input: serde_json::json!({}),
            }],
            origin: None,
        },
        Message {
            id: None,
            role: MessageRole::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "tc-cancel".into(),
                content: "Interrupted by user".into(),
                is_error: true,
                metadata: Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
            }],
            origin: None,
        },
    ];
    let display = project_messages(&msgs);
    let tc = &display[0].tool_calls[0];
    let metadata = tc.metadata.as_ref().expect("cancel_cause must survive");
    match metadata {
        ToolResultMetadata::Cancelled { cause } => assert_eq!(*cause, CancelCause::UserInterrupt),
        other => panic!("expected Cancelled, got {other:?}"),
    }
}
