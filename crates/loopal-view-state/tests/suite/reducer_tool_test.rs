//! Tool / token / mode / turn metric tests.

use loopal_protocol::AgentEventPayload;
use loopal_view_state::ViewStateReducer;

#[test]
fn tool_call_increments_count_and_in_flight_and_records_last_tool() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::ToolCall {
        id: "1".into(),
        name: "Bash".into(),
        input: serde_json::json!({}),
    });
    let obs = &r.state().agent.observable;
    assert_eq!(obs.tool_count, 1);
    assert_eq!(obs.tools_in_flight, 1);
    assert_eq!(obs.last_tool.as_deref(), Some("Bash"));
}

#[test]
fn tool_result_decrements_in_flight() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::ToolCall {
        id: "1".into(),
        name: "Bash".into(),
        input: serde_json::json!({}),
    });
    r.apply(AgentEventPayload::ToolResult {
        id: "1".into(),
        name: "Bash".into(),
        result: "ok".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });
    assert_eq!(r.state().agent.observable.tools_in_flight, 0);
}

#[test]
fn tool_result_does_not_underflow_when_no_call_in_flight() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::ToolResult {
        id: "1".into(),
        name: "Bash".into(),
        result: "ok".into(),
        is_error: false,
        duration_ms: None,
        metadata: None,
    });
    assert_eq!(r.state().agent.observable.tools_in_flight, 0);
}

#[test]
fn token_usage_replaces_input_output_counters() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::TokenUsage {
        input_tokens: 100,
        output_tokens: 50,
        context_window: 200_000,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
        thinking_tokens: 0,
    });
    let obs = &r.state().agent.observable;
    assert_eq!(obs.input_tokens, 100);
    assert_eq!(obs.output_tokens, 50);
}

#[test]
fn mode_changed_updates_mode_string() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::ModeChanged {
        mode: "plan".into(),
    });
    assert_eq!(r.state().agent.observable.mode, "plan");
}

#[test]
fn turn_completed_increments_turn_count() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::TurnCompleted {
        turn_id: 1,
        duration_ms: 100,
        llm_calls: 1,
        tool_calls_requested: 0,
        tool_calls_approved: 0,
        tool_calls_denied: 0,
        tool_errors: 0,
        auto_continuations: 0,
        warnings_injected: 0,
        tokens_in: 0,
        tokens_out: 0,
        modified_files: vec![],
    });
    assert_eq!(r.state().agent.observable.turn_count, 1);
}
