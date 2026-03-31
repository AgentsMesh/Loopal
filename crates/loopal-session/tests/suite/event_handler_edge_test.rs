//! Edge cases for ToolResult handling: summary preservation.

use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_session::ToolCallStatus;
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

macro_rules! conv {
    ($state:expr) => {
        &$state.agents["main"].conversation
    };
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
    let summary_before = conv!(state).messages[0].tool_calls[0].summary.clone();

    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolResult {
            id: "tc-1".into(),
            name: "Read".into(),
            result: "file contents here".into(),
            is_error: false,
            duration_ms: None,

            metadata: None,
        }),
    );

    assert_eq!(
        conv!(state).messages[0].tool_calls[0].summary,
        summary_before
    );
    assert!(
        conv!(state).messages[0].tool_calls[0]
            .summary
            .contains("Read")
    );
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

            metadata: None,
        }),
    );

    let tc = &conv!(state).messages[0].tool_calls[0];
    assert_eq!(tc.status, ToolCallStatus::Success);
    assert_eq!(tc.result, Some("hello\nworld".into()));
}

