//! Tests for the "recently active" grace window used by the TUI status
//! bar to bridge the gap between `AwaitingInput` and `Running` events.

use std::time::Duration;

use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_session::event_handler::apply_event;
use loopal_session::state::{ROOT_AGENT, SessionState};

fn make_state() -> SessionState {
    SessionState::new("test-model".into(), "act".into())
}

#[test]
fn is_recently_active_true_after_activity_event() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::Stream { text: "hi".into() }),
    );
    assert!(
        state.agents[ROOT_AGENT]
            .conversation
            .is_recently_active(Duration::from_secs(1)),
        "Stream event must stamp last_active_at",
    );
}

#[test]
fn is_recently_active_false_on_fresh_state() {
    let state = make_state();
    assert!(
        !state.agents[ROOT_AGENT]
            .conversation
            .is_recently_active(Duration::from_millis(500)),
        "a fresh conversation has no activity stamp",
    );
}

#[test]
fn awaiting_input_keeps_recent_activity_from_prior_stream() {
    // This is the exact scenario the fix targets: a Stream event stamps
    // activity, then AwaitingInput (end of turn) arrives and clears the
    // turn timer — but the grace window remains open so the TUI can keep
    // its spinner alive until the next Running lands.
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::Stream {
            text: "working".into(),
        }),
    );
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::AwaitingInput),
    );
    let conv = &state.agents[ROOT_AGENT].conversation;
    // Turn timer cleared (AwaitingInput calls end_turn).
    assert!(
        !conv.is_recently_active(Duration::from_nanos(0)),
        "zero-grace window is always expired",
    );
    assert!(
        conv.is_recently_active(Duration::from_secs(1)),
        "1s grace must still cover the Stream from moments ago",
    );
}

#[test]
fn running_event_stamps_activity() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Running));
    assert!(
        state.agents[ROOT_AGENT]
            .conversation
            .is_recently_active(Duration::from_secs(1)),
    );
}

#[test]
fn tool_call_stamps_activity() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::ToolCall {
            id: "t1".into(),
            name: "Read".into(),
            input: serde_json::json!({}),
        }),
    );
    assert!(
        state.agents[ROOT_AGENT]
            .conversation
            .is_recently_active(Duration::from_secs(1)),
    );
}

#[test]
fn reset_timer_clears_activity() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Running));
    state
        .agents
        .get_mut(ROOT_AGENT)
        .unwrap()
        .conversation
        .reset_timer();
    assert!(
        !state.agents[ROOT_AGENT]
            .conversation
            .is_recently_active(Duration::from_secs(1)),
    );
}
