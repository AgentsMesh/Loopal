//! Status-transition events: Started/Running/AwaitingInput/Finished/Error.

use loopal_protocol::{AgentEventPayload, AgentStatus, ContinuationGateSummary, GateCloseReason};
use loopal_view_state::ViewStateReducer;

fn assert_status(reducer: &ViewStateReducer, expected: AgentStatus) {
    assert_eq!(reducer.state().agent.observable.status, expected);
}

fn completed_turn() -> AgentEventPayload {
    AgentEventPayload::TurnCompleted(loopal_protocol::TurnSummary {
        turn_id: 1,
        duration_ms: 10,
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
    })
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
fn provider_warning_is_visible_without_poisoning_session_status() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);
    r.apply(AgentEventPayload::Stream {
        text: "partial".into(),
    });
    r.apply(AgentEventPayload::ProviderWarning {
        message: "stream interrupted; continuing".into(),
    });

    assert_status(&r, AgentStatus::Running);
    let messages = &r.state().agent.conversation.messages;
    assert_eq!(messages[0].role, "assistant");
    assert_eq!(messages[0].content, "partial");
    assert_eq!(messages[1].role, "system");
    assert_eq!(messages[1].content, "stream interrupted; continuing");
}

#[test]
fn turn_completed_is_the_only_turn_counter_authority() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);
    r.apply(completed_turn());
    r.apply(AgentEventPayload::AwaitingInput);
    r.apply(AgentEventPayload::AwaitingInput);

    assert_status(&r, AgentStatus::WaitingForInput);
    assert_eq!(r.state().agent.observable.turn_count, 1);
    assert_eq!(r.state().agent.conversation.turn_count, 1);
}

#[test]
fn finished_does_not_replace_an_error_terminal_state() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::Running);
    let error_rev = r
        .apply(AgentEventPayload::Error {
            message: "boom".into(),
        })
        .expect("error mutates state");

    assert_eq!(r.apply(AgentEventPayload::Finished), None);
    assert_eq!(r.rev(), error_rev);
    assert_status(&r, AgentStatus::Error);
}

#[test]
fn observable_event_returns_new_rev() {
    let mut r = ViewStateReducer::new("root");
    let new_rev = r.apply(AgentEventPayload::Running).expect("observable");
    assert_eq!(new_rev, 1);
}

#[test]
fn user_suspend_gate_projects_suspended_and_reopened_status() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::AwaitingInput);
    r.apply(AgentEventPayload::ContinuationGateChanged(
        ContinuationGateSummary {
            open: false,
            closed_reason: Some(GateCloseReason::UserSuspend),
            wake_deadline: None,
        },
    ));
    assert_status(&r, AgentStatus::Suspended);

    r.apply(AgentEventPayload::ContinuationGateChanged(
        ContinuationGateSummary {
            open: true,
            closed_reason: None,
            wake_deadline: None,
        },
    ));
    assert_status(&r, AgentStatus::Running);
}

#[test]
fn model_requested_idle_gate_keeps_waiting_status() {
    let mut r = ViewStateReducer::new("root");
    r.apply(AgentEventPayload::AwaitingInput);
    let revision = r.rev();
    assert_eq!(
        r.apply(AgentEventPayload::ContinuationGateChanged(
            ContinuationGateSummary {
                open: false,
                closed_reason: Some(GateCloseReason::ModelRequested),
                wake_deadline: None,
            },
        )),
        None
    );
    assert_eq!(r.rev(), revision);
    assert_status(&r, AgentStatus::WaitingForInput);
}
