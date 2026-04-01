//! Full agent lifecycle tracking through SessionState.
//! Tests normal business flows: spawn -> work -> complete.

use loopal_protocol::{AgentEvent, AgentEventPayload, AgentStatus};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

pub(crate) fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

/// Helper: apply a sequence of named-agent events.
pub(crate) fn apply_sequence(
    state: &mut SessionState,
    agent: &str,
    payloads: Vec<AgentEventPayload>,
) {
    for payload in payloads {
        apply_event(state, AgentEvent::named(agent, payload));
    }
}

// ── Full lifecycle ───────────────────────────────────────────────────

/// Complete sub-agent lifecycle: Started -> tools -> stream -> token usage -> Finished.
#[test]
fn full_lifecycle_started_to_finished() {
    let mut state = make_state();
    apply_sequence(
        &mut state,
        "researcher",
        vec![
            AgentEventPayload::Started,
            AgentEventPayload::ToolCall {
                id: "tc-1".into(),
                name: "Glob".into(),
                input: serde_json::json!({"pattern": "*.rs"}),
            },
            AgentEventPayload::ToolResult {
                id: "tc-1".into(),
                name: "Glob".into(),
                result: "found 10 files".into(),
                is_error: false,
                duration_ms: Some(15),

                metadata: None,
            },
            AgentEventPayload::ToolCall {
                id: "tc-2".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "/src/main.rs"}),
            },
            AgentEventPayload::ToolResult {
                id: "tc-2".into(),
                name: "Read".into(),
                result: "fn main() {}".into(),
                is_error: false,
                duration_ms: Some(3),

                metadata: None,
            },
            AgentEventPayload::Stream {
                text: "Analysis: ".into(),
            },
            AgentEventPayload::Stream {
                text: "the project uses Rust.".into(),
            },
            AgentEventPayload::TokenUsage {
                input_tokens: 5000,
                output_tokens: 1200,
                context_window: 200_000,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                thinking_tokens: 0,
            },
            AgentEventPayload::Finished,
        ],
    );

    let agent = &state.agents["researcher"];
    assert_eq!(agent.observable.status, AgentStatus::Finished);
    assert_eq!(agent.observable.tool_count, 2);
    assert_eq!(agent.observable.tools_in_flight, 0);
    assert_eq!(agent.observable.input_tokens, 5000);
    assert_eq!(agent.observable.output_tokens, 1200);
    assert!(agent.started_at.is_some());
}

/// tools_in_flight increments on ToolCall and decrements on ToolResult.
#[test]
fn tools_in_flight_balances_correctly() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );

    // Two parallel tool calls
    for id in ["tc-a", "tc-b"] {
        apply_event(
            &mut state,
            AgentEvent::named(
                "w1",
                AgentEventPayload::ToolCall {
                    id: id.into(),
                    name: "Ls".into(),
                    input: serde_json::json!({}),
                },
            ),
        );
    }
    assert_eq!(state.agents["w1"].observable.tools_in_flight, 2);

    // First result arrives
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::ToolResult {
                id: "tc-a".into(),
                name: "Ls".into(),
                result: "ok".into(),
                is_error: false,
                duration_ms: None,

                metadata: None,
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.tools_in_flight, 1);

    // Second result
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::ToolResult {
                id: "tc-b".into(),
                name: "Ls".into(),
                result: "ok".into(),
                is_error: false,
                duration_ms: None,

                metadata: None,
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.tools_in_flight, 0);
    assert_eq!(state.agents["w1"].observable.tool_count, 2);
}
