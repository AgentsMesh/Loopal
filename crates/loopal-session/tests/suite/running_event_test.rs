//! Tests for `AgentEventPayload::Running` — turn-begin authoritative signal.

use std::time::Duration;

use loopal_protocol::{AgentEvent, AgentEventPayload, AgentStatus};
use loopal_session::event_handler::apply_event;
use loopal_session::state::{ROOT_AGENT, SessionState};

fn make_state() -> SessionState {
    SessionState::new("test-model".into(), "act".into())
}

#[test]
fn running_flips_status_to_running() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::AwaitingInput),
    );
    assert_eq!(
        state.agents[ROOT_AGENT].observable.status,
        AgentStatus::WaitingForInput,
    );

    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Running));
    assert_eq!(
        state.agents[ROOT_AGENT].observable.status,
        AgentStatus::Running,
    );
}

#[test]
fn running_starts_turn_timer() {
    let mut state = make_state();
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Running));
    // After Running, turn_elapsed starts accumulating (> 0 after a short sleep).
    std::thread::sleep(Duration::from_millis(5));
    let elapsed = state.active_conversation().turn_elapsed();
    assert!(
        elapsed >= Duration::from_millis(1),
        "expected turn timer running, got {elapsed:?}",
    );
}

#[test]
fn running_on_sub_agent_creates_view() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("worker", AgentEventPayload::Running),
    );
    assert!(state.agents.contains_key("worker"));
    assert_eq!(
        state.agents["worker"].observable.status,
        AgentStatus::Running,
    );
}
