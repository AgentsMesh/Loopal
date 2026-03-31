//! Tests for ProjectedMessage → SessionMessage conversion via load_display_history.

use loopal_session::{ROOT_AGENT, ToolCallStatus};

use super::controller_test::make_controller;

// ── ProjectedMessage → SessionMessage conversion ────────────────

#[test]
fn load_display_history_converts_projected_to_session() {
    let (ctrl, _control_rx, _perm_rx) = make_controller();

    let projected = vec![
        loopal_protocol::ProjectedMessage {
            role: "user".into(),
            content: "hello".into(),
            tool_calls: vec![],
            image_count: 0,
        },
        loopal_protocol::ProjectedMessage {
            role: "assistant".into(),
            content: "response".into(),
            tool_calls: vec![loopal_protocol::ProjectedToolCall {
                id: "t1".into(),
                name: "Read".into(),
                summary: "Read(file.rs)".into(),
                result: Some("file contents".into()),
                is_error: false,
                input: None,
                metadata: None,
            }],
            image_count: 0,
        },
    ];

    ctrl.load_display_history(projected);

    let state = ctrl.lock();
    let msgs = &state.agents[ROOT_AGENT].conversation.messages;
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, "user");
    assert_eq!(msgs[0].content, "hello");
    assert_eq!(msgs[1].role, "assistant");
    assert_eq!(msgs[1].tool_calls.len(), 1);
    assert_eq!(msgs[1].tool_calls[0].name, "Read");
    assert_eq!(msgs[1].tool_calls[0].status, ToolCallStatus::Success);
    assert_eq!(
        msgs[1].tool_calls[0].result.as_deref(),
        Some("file contents")
    );
}

#[test]
fn load_display_history_error_tool_gets_error_status() {
    let (ctrl, _control_rx, _perm_rx) = make_controller();

    let projected = vec![loopal_protocol::ProjectedMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: vec![loopal_protocol::ProjectedToolCall {
            id: "t1".into(),
            name: "Bash".into(),
            summary: "Bash(cmd)".into(),
            result: Some("command failed".into()),
            is_error: true,
            input: None,
            metadata: None,
        }],
        image_count: 0,
    }];

    ctrl.load_display_history(projected);

    let state = ctrl.lock();
    let msgs = &state.agents[ROOT_AGENT].conversation.messages;
    assert_eq!(msgs[0].tool_calls[0].status, ToolCallStatus::Error);
}

#[test]
fn load_display_history_pending_tool_no_result() {
    let (ctrl, _control_rx, _perm_rx) = make_controller();

    let projected = vec![loopal_protocol::ProjectedMessage {
        role: "assistant".into(),
        content: String::new(),
        tool_calls: vec![loopal_protocol::ProjectedToolCall {
            id: "t1".into(),
            name: "Write".into(),
            summary: "Write(file)".into(),
            result: None,
            is_error: false,
            input: None,
            metadata: None,
        }],
        image_count: 0,
    }];

    ctrl.load_display_history(projected);

    let state = ctrl.lock();
    let msgs = &state.agents[ROOT_AGENT].conversation.messages;
    assert_eq!(msgs[0].tool_calls[0].status, ToolCallStatus::Pending);
}
