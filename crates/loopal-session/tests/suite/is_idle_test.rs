//! Tests for AgentViewState::is_idle() correctness mapping.
//!
//! Exhaustively verifies each AgentStatus variant maps to the correct
//! idle/busy classification. is_idle() replaces the former `agent_idle` flag.

use loopal_protocol::AgentStatus;
use loopal_session::state::AgentViewState;

fn view_with_status(status: AgentStatus) -> AgentViewState {
    let mut view = AgentViewState::default();
    view.observable.status = status;
    view
}

#[test]
fn test_is_idle_returns_true_for_waiting_for_input() {
    assert!(view_with_status(AgentStatus::WaitingForInput).is_idle());
}

#[test]
fn test_is_idle_returns_true_for_finished() {
    assert!(view_with_status(AgentStatus::Finished).is_idle());
}

#[test]
fn test_is_idle_returns_true_for_error() {
    assert!(view_with_status(AgentStatus::Error).is_idle());
}

#[test]
fn test_is_idle_returns_false_for_starting() {
    assert!(!view_with_status(AgentStatus::Starting).is_idle());
}

#[test]
fn test_is_idle_returns_false_for_running() {
    assert!(!view_with_status(AgentStatus::Running).is_idle());
}
