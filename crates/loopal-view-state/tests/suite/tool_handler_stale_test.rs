use loopal_protocol::AgentEventPayload;
use loopal_tool_invocation::{CancelCause, InvocationState, StaleReason, ToolResultMetadata};
use loopal_view_state::ViewStateReducer;

fn tool_call(id: &str, name: &str) -> AgentEventPayload {
    AgentEventPayload::ToolCall {
        id: id.into(),
        name: name.into(),
        input: serde_json::json!({}),
    }
}

#[test]
fn tool_result_with_user_interrupt_metadata_transitions_to_cancelled() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("c1", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "c1".into(),
        name: "Bash".into(),
        result: "Interrupted by user".into(),
        is_error: true,
        duration_ms: None,
        metadata: Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    let InvocationState::Cancelled { cause, .. } = &tc.state else {
        panic!("expected Cancelled, got {:?}", tc.state.variant_name())
    };
    assert_eq!(*cause, CancelCause::UserInterrupt);
}

#[test]
fn tool_result_with_parent_cancelled_metadata_transitions_to_cancelled() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("c2", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "c2".into(),
        name: "Bash".into(),
        result: "Cancelled".into(),
        is_error: true,
        duration_ms: None,
        metadata: Some(ToolResultMetadata::cancelled(CancelCause::ParentCancelled)),
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    let InvocationState::Cancelled { cause, .. } = &tc.state else {
        panic!("expected Cancelled")
    };
    assert_eq!(*cause, CancelCause::ParentCancelled);
}

#[test]
fn tool_result_with_watchdog_stale_reason_transitions_to_stale() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("a", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "a".into(),
        name: "Bash".into(),
        result: "Watchdog timeout".into(),
        is_error: true,
        duration_ms: None,
        metadata: Some(ToolResultMetadata::stale(StaleReason::WatchdogTimeout)),
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    let InvocationState::Stale { reason, .. } = &tc.state else {
        panic!("expected Stale, got {:?}", tc.state.variant_name())
    };
    assert_eq!(*reason, StaleReason::WatchdogTimeout);
}

#[test]
fn tool_result_with_turn_ended_reason_transitions_to_stale() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("b", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "b".into(),
        name: "Bash".into(),
        result: "x".into(),
        is_error: true,
        duration_ms: None,
        metadata: Some(ToolResultMetadata::stale(StaleReason::TurnEnded)),
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    let InvocationState::Stale { reason, .. } = &tc.state else {
        panic!("expected Stale")
    };
    assert_eq!(*reason, StaleReason::TurnEnded);
}

#[test]
fn tool_result_bytes_written_metadata_transitions_to_done() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("c", "Write"));
    r.apply(AgentEventPayload::ToolResult {
        id: "c".into(),
        name: "Write".into(),
        result: "ok".into(),
        is_error: false,
        duration_ms: None,
        metadata: Some(ToolResultMetadata::bytes_written(42)),
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc.state, InvocationState::Done { .. }));
    assert_eq!(
        tc.metadata.as_ref(),
        Some(&ToolResultMetadata::bytes_written(42))
    );
}

#[test]
fn tool_result_no_metadata_transitions_to_done() {
    let mut r = ViewStateReducer::new("main");
    r.apply(tool_call("d", "Bash"));
    r.apply(AgentEventPayload::ToolResult {
        id: "d".into(),
        name: "Bash".into(),
        result: "ok".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });

    let tc = &r.state().agent.conversation.messages[0].tool_calls[0];
    assert!(matches!(tc.state, InvocationState::Done { .. }));
    assert!(tc.metadata.is_none());
}
