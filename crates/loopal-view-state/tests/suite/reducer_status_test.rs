//! Status-transition events: Started/Running/AwaitingInput/Finished/Error.

use loopal_protocol::{AgentEventPayload, AgentStatus};
use loopal_view_state::ViewStateReducer;

fn assert_status(reducer: &ViewStateReducer, expected: AgentStatus) {
    assert_eq!(reducer.state().agent.observable.status, expected);
}

#[test]
fn started_sets_running() {
    let mut r = ViewStateReducer::new("root");
    let delta = r.apply(AgentEventPayload::Started);
    assert!(delta.is_some());
    assert_status(&r, AgentStatus::Running);
}

#[test]
fn running_sets_running() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);
    assert_status(&r, AgentStatus::Running);
}

#[test]
fn awaiting_input_sets_waiting_for_input() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::AwaitingInput);
    assert_status(&r, AgentStatus::WaitingForInput);
}

#[test]
fn finished_sets_finished() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Finished);
    assert_status(&r, AgentStatus::Finished);
}

#[test]
fn error_event_sets_error_status() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Error {
        message: "boom".into(),
    });
    assert_status(&r, AgentStatus::Error);
}

#[test]
fn observable_event_returns_new_rev() {
    let mut r = ViewStateReducer::new("root");
    let new_rev = r.apply(AgentEventPayload::Running).expect("observable");
    assert_eq!(new_rev, 1);
}
