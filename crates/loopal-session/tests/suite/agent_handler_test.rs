//! Tests for agent_handler: apply_agent_event updating AgentViewState.

use loopal_protocol::AgentStatus;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

#[test]
fn test_started_creates_agent_view_state() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    assert!(state.agents.contains_key("w1"));
    assert_eq!(state.agents["w1"].observable.status, AgentStatus::Running);
}

#[test]
fn test_tool_call_increments_count() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::ToolCall {
                id: "tc1".into(),
                name: "Read".into(),
                input: serde_json::json!({}),
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.tool_count, 1);
    assert_eq!(
        state.agents["w1"].observable.last_tool,
        Some("Read".to_string())
    );
}

#[test]
fn test_tool_result_keeps_running() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::ToolResult {
                id: "tc1".into(),
                name: "Read".into(),
                result: "ok".into(),
                is_error: false,
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.status, AgentStatus::Running);
}

#[test]
fn test_token_usage_updates_agent() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::TokenUsage {
                input_tokens: 500,
                output_tokens: 200,
                context_window: 100_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                thinking_tokens: 0,
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.input_tokens, 500);
    assert_eq!(state.agents["w1"].observable.output_tokens, 200);
}

#[test]
fn test_awaiting_input_sets_waiting() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::AwaitingInput),
    );
    assert_eq!(
        state.agents["w1"].observable.status,
        AgentStatus::WaitingForInput
    );
    assert_eq!(state.agents["w1"].observable.turn_count, 1);
}

#[test]
fn test_finished_sets_finished() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Finished),
    );
    assert_eq!(state.agents["w1"].observable.status, AgentStatus::Finished);
}

#[test]
fn test_error_sets_error_status() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::Error {
                message: "boom".into(),
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.status, AgentStatus::Error);
}

#[test]
fn test_mode_changed_updates_agent_mode() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::ModeChanged {
                mode: "plan".into(),
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.mode, "plan");
}

#[test]
fn test_stream_keeps_running() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::Stream {
                text: "thinking".into(),
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.status, AgentStatus::Running);
}
