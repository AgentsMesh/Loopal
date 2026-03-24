//! Edge cases for ToolResult handling: summary preservation, AttemptCompletion promotion.

use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_session::ToolCallStatus;
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;
use loopal_tool_api::COMPLETION_PREFIX;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

#[test]
fn test_tool_result_preserves_input_summary() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolCall {
            id: "tc-1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "/tmp/foo.rs"}),
        }),
    );
    let summary_before = state.messages[0].tool_calls[0].summary.clone();

    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolResult {
            id: "tc-1".into(),
            name: "Read".into(),
            result: "file contents here".into(),
            is_error: false,
            duration_ms: None,
        }),
    );

    // summary must NOT be overwritten by the result text
    assert_eq!(state.messages[0].tool_calls[0].summary, summary_before);
    assert!(state.messages[0].tool_calls[0].summary.contains("Read"));
}

#[test]
fn test_tool_result_stores_full_content() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolCall {
            id: "tc-2".into(),
            name: "Bash".into(),
            input: serde_json::json!({"command": "echo hello"}),
        }),
    );
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolResult {
            id: "tc-2".into(),
            name: "Bash".into(),
            result: "hello\nworld".into(),
            is_error: false,
            duration_ms: None,
        }),
    );

    let tc = &state.messages[0].tool_calls[0];
    assert_eq!(tc.status, ToolCallStatus::Success);
    assert_eq!(tc.result, Some("hello\nworld".into()));
}

#[test]
fn test_attempt_completion_promotes_to_assistant_message() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolCall {
            id: "tc-ac".into(),
            name: "AttemptCompletion".into(),
            input: serde_json::json!({"result": "# Report\n\nDone."}),
        }),
    );
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolResult {
            id: "tc-ac".into(),
            name: "AttemptCompletion".into(),
            result: format!("{COMPLETION_PREFIX}# Report\n\nDone."),
            is_error: false,
            duration_ms: None,
        }),
    );

    // Tool call should not store result (content promoted)
    let tc = &state.messages[0].tool_calls[0];
    assert_eq!(tc.status, ToolCallStatus::Success);
    assert!(tc.result.is_none());
    assert_eq!(tc.summary, "AttemptCompletion"); // no ugly JSON dump

    // Content promoted to a standalone assistant message
    assert_eq!(state.messages.len(), 2);
    assert_eq!(state.messages[1].role, "assistant");
    assert_eq!(state.messages[1].content, "# Report\n\nDone.");
    assert!(state.messages[1].tool_calls.is_empty());
}

#[test]
fn test_attempt_completion_error_not_promoted() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolCall {
            id: "tc-err".into(),
            name: "AttemptCompletion".into(),
            input: serde_json::json!({"result": "oops"}),
        }),
    );
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolResult {
            id: "tc-err".into(),
            name: "AttemptCompletion".into(),
            result: "something went wrong".into(),
            is_error: true,
            duration_ms: None,
        }),
    );

    // Error result should be stored normally, not promoted
    let tc = &state.messages[0].tool_calls[0];
    assert_eq!(tc.status, ToolCallStatus::Error);
    assert!(tc.result.is_some());
    // No additional assistant message created
    assert_eq!(state.messages.len(), 1);
}
