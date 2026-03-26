//! Tests for RetryError/RetryCleared event handling (retry banner).

use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_session::event_handler::apply_event;
use loopal_session::state::SessionState;

fn make_state() -> SessionState {
    SessionState::new("test-model".to_string(), "act".to_string())
}

#[test]
fn test_retry_error_sets_banner() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::RetryError {
            message: "502 Bad Gateway. Retrying in 2.0s".into(),
            attempt: 1,
            max_attempts: 6,
        }),
    );
    assert_eq!(
        state.retry_banner.as_deref(),
        Some("502 Bad Gateway. Retrying in 2.0s (1/6)")
    );
}

#[test]
fn test_retry_error_does_not_add_message() {
    let mut state = make_state();
    let before = state.messages.len();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::RetryError {
            message: "error".into(),
            attempt: 1,
            max_attempts: 6,
        }),
    );
    assert_eq!(
        state.messages.len(),
        before,
        "RetryError must not append to messages"
    );
}

#[test]
fn test_retry_cleared_clears_banner() {
    let mut state = make_state();
    state.retry_banner = Some("old error".into());
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::RetryCleared),
    );
    assert!(state.retry_banner.is_none());
}

#[test]
fn test_error_clears_retry_banner() {
    let mut state = make_state();
    state.retry_banner = Some("transient".into());
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::Error {
            message: "permanent".into(),
        }),
    );
    assert!(state.retry_banner.is_none());
}

#[test]
fn test_awaiting_input_clears_retry_banner() {
    let mut state = make_state();
    state.retry_banner = Some("retrying".into());
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::AwaitingInput),
    );
    assert!(state.retry_banner.is_none());
}

#[test]
fn test_retry_updates_in_place() {
    let mut state = make_state();
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::RetryError {
            message: "502. Retrying in 2.0s".into(),
            attempt: 1,
            max_attempts: 6,
        }),
    );
    apply_event(
        &mut state,
        AgentEvent::root(AgentEventPayload::RetryError {
            message: "502. Retrying in 4.0s".into(),
            attempt: 2,
            max_attempts: 6,
        }),
    );
    // Only one banner, updated in place
    assert_eq!(
        state.retry_banner.as_deref(),
        Some("502. Retrying in 4.0s (2/6)")
    );
    // No messages added
    assert_eq!(state.messages.len(), 0);
}

#[test]
fn test_finished_clears_retry_banner() {
    let mut state = make_state();
    state.retry_banner = Some("retrying".into());
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Finished));
    assert!(state.retry_banner.is_none());
}

#[test]
fn test_interrupted_clears_retry_banner() {
    let mut state = make_state();
    state.retry_banner = Some("retrying".into());
    apply_event(&mut state, AgentEvent::root(AgentEventPayload::Interrupted));
    assert!(state.retry_banner.is_none());
}
