use loopal_protocol::*;

use crate::workflow_support::*;

fn assert_illegal(run: &WorkflowRunSnapshot, payload: WorkflowEventPayload) {
    assert!(matches!(
        reduce_workflow_event(run, &event(run, payload), &AcceptJson),
        Err(WorkflowReduceError::IllegalTransition { .. })
    ));
}

#[test]
fn repeated_cancel_is_rejected_while_an_attempt_is_cancelling() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let run = apply(&run, WorkflowEventPayload::CancelRequested { reason: None });

    assert_eq!(run.state, WorkflowRunState::Cancelling);
    assert_illegal(
        &run,
        WorkflowEventPayload::CancelRequested {
            reason: Some("duplicate".into()),
        },
    );
}

#[test]
fn attempt_cancel_requires_a_cancelling_run() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");

    assert_illegal(
        &run,
        WorkflowEventPayload::AttemptCancelled {
            node_id: "source".into(),
            attempt_id: "watt_source".into(),
            reason: "unexpected".into(),
        },
    );
}

#[test]
fn attempt_cancel_requires_a_cancelling_attempt() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let mut run = apply(&run, WorkflowEventPayload::CancelRequested { reason: None });
    run.attempts[0].state = WorkflowAttemptState::Running;

    assert_illegal(
        &run,
        WorkflowEventPayload::AttemptCancelled {
            node_id: "source".into(),
            attempt_id: "watt_source".into(),
            reason: "state mismatch".into(),
        },
    );
}

#[test]
fn failure_during_cancellation_fails_without_reopening_admission() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let run = apply(&run, WorkflowEventPayload::CancelRequested { reason: None });
    let run = apply(
        &run,
        WorkflowEventPayload::AttemptFailed {
            node_id: "source".into(),
            attempt_id: "watt_source".into(),
            completion: AgentCompletion::new("transport_error", None),
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::TransientBeforeExecution,
                reason: "late transport failure".into(),
            },
        },
    );
    assert_eq!(run.state, WorkflowRunState::Failed);
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Failed);
    assert!(run.nodes.iter().all(|node| node.state.is_terminal()));
}

#[test]
fn cancellation_preserves_nodes_that_already_succeeded() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let run = succeed(&run, "source", "watt_source", None);
    let run = apply(&run, WorkflowEventPayload::CancelRequested { reason: None });

    assert_eq!(run.state, WorkflowRunState::Cancelled);
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Succeeded);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Cancelled);
}

fn stop(node: &str, attempt: &str) -> WorkflowEventPayload {
    WorkflowEventPayload::AttemptStopRequested {
        node_id: node.into(),
        attempt_id: attempt.into(),
        reason: "stop requested".into(),
    }
}

fn deadline_failure() -> WorkflowAttemptFailure {
    WorkflowAttemptFailure {
        class: WorkflowFailureClass::Permanent,
        reason: "deadline exceeded".into(),
    }
}

#[test]
fn attempt_stop_accepts_dispatching_and_running_attempts() {
    let dispatching = dispatch(&running(text_spec()), "source", "watt_dispatching");
    let stopped = apply(&dispatching, stop("source", "watt_dispatching"));
    assert_eq!(stopped.nodes[0].state, WorkflowNodeState::Cancelling);
    assert_eq!(stopped.attempts[0].state, WorkflowAttemptState::Cancelling);

    let running_attempt = bind_and_run(&dispatching, "source", "watt_dispatching");
    let stopped = apply(&running_attempt, stop("source", "watt_dispatching"));
    assert_eq!(stopped.nodes[0].state, WorkflowNodeState::Cancelling);
    assert_eq!(stopped.attempts[0].state, WorkflowAttemptState::Cancelling);
}

#[test]
fn attempt_stop_rejects_nonrunning_run_node_and_attempt_states() {
    let planned = planned(text_spec());
    assert_illegal(&planned, stop("source", "watt_missing"));

    let ready = running(text_spec());
    assert_illegal(&ready, stop("source", "watt_missing"));

    let mut inconsistent = dispatch(&ready, "source", "watt_inconsistent");
    inconsistent.attempts[0].state = WorkflowAttemptState::Succeeded;
    assert_illegal(&inconsistent, stop("source", "watt_inconsistent"));
}

#[test]
fn deadline_requires_a_running_run_without_active_attempts() {
    let planned = planned(text_spec());
    assert_illegal(
        &planned,
        WorkflowEventPayload::RunDeadlineExceeded {
            failure: deadline_failure(),
        },
    );

    let running = running(text_spec());
    let active = dispatch(&running, "source", "watt_active");
    assert_illegal(
        &active,
        WorkflowEventPayload::RunDeadlineExceeded {
            failure: deadline_failure(),
        },
    );

    let failed = apply(
        &running,
        WorkflowEventPayload::RunDeadlineExceeded {
            failure: deadline_failure(),
        },
    );
    assert_eq!(failed.state, WorkflowRunState::Failed);
    assert_eq!(failed.failure, Some(deadline_failure()));
    assert!(failed.nodes.iter().all(|node| node.state.is_terminal()));
}
