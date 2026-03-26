//! Edge-case tests for agent_handler: RetryError/RetryCleared on sub-agents.

use loopal_protocol::AgentStatus;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

#[test]
fn test_retry_error_keeps_running() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named(
            "w1",
            AgentEventPayload::RetryError {
                message: "502".into(),
                attempt: 1,
                max_attempts: 6,
            },
        ),
    );
    assert_eq!(state.agents["w1"].observable.status, AgentStatus::Running);
}

#[test]
fn test_retry_cleared_no_crash() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::Started),
    );
    apply_event(
        &mut state,
        AgentEvent::named("w1", AgentEventPayload::RetryCleared),
    );
    // Status unchanged from Started → Running
    assert_eq!(state.agents["w1"].observable.status, AgentStatus::Running);
}
