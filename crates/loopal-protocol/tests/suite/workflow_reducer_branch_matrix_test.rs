use loopal_protocol::*;

use crate::workflow_support::*;

#[path = "workflow_reducer_branch_matrix_test/completion.rs"]
mod completion;

fn digest() -> WorkflowAttemptCapabilityDigest {
    WorkflowAttemptCapability::parse("77".repeat(32))
        .unwrap()
        .digest()
}

fn reduce(
    run: &WorkflowRunSnapshot,
    payload: WorkflowEventPayload,
) -> Result<WorkflowReduceOutcome, WorkflowReduceError> {
    reduce_workflow_event(run, &event(run, payload), &AcceptJson)
}

fn dispatch_event(node: &str, attempt: &str) -> WorkflowEventPayload {
    WorkflowEventPayload::DispatchIntended {
        node_id: node.into(),
        attempt_id: attempt.into(),
        capability_digest: digest(),
    }
}

fn bound_event(node: &str, attempt: &str) -> WorkflowEventPayload {
    WorkflowEventPayload::AttemptBound {
        node_id: node.into(),
        attempt_id: attempt.into(),
        agent: QualifiedAddress::local("coverage-worker"),
    }
}

fn running_event(node: &str, attempt: &str) -> WorkflowEventPayload {
    WorkflowEventPayload::AttemptRunning {
        node_id: node.into(),
        attempt_id: attempt.into(),
    }
}

fn failed_event(node: &str, attempt: &str, completion: AgentCompletion) -> WorkflowEventPayload {
    WorkflowEventPayload::AttemptFailed {
        node_id: node.into(),
        attempt_id: attempt.into(),
        completion,
        failure: WorkflowAttemptFailure {
            class: WorkflowFailureClass::Permanent,
            reason: "coverage failure".into(),
        },
    }
}

fn assert_illegal(result: Result<WorkflowReduceOutcome, WorkflowReduceError>) {
    assert!(matches!(
        result,
        Err(WorkflowReduceError::IllegalTransition { .. })
    ));
}

#[test]
fn validation_and_dispatch_rejection_matrix_is_fail_closed() {
    let run = running(text_spec());
    assert_illegal(reduce(&run, WorkflowEventPayload::SpecValidated));

    let dispatched = dispatch(&run, "source", "watt_existing");
    assert_eq!(
        reduce(&dispatched, dispatch_event("source", "watt_existing")),
        Err(WorkflowReduceError::AttemptExists)
    );

    let mut exhausted = run.clone();
    exhausted.spec.limits.max_attempts = 0;
    assert_eq!(
        reduce(&exhausted, dispatch_event("source", "watt_exhausted")),
        Err(WorkflowReduceError::AttemptsExhausted)
    );

    let mut saturated = run.clone();
    saturated.spec.limits.max_parallel = 0;
    assert_illegal(reduce(
        &saturated,
        dispatch_event("source", "watt_saturated"),
    ));

    assert_illegal(reduce(&run, dispatch_event("output", "watt_not_ready")));
}

#[test]
fn binding_checks_node_attempt_and_agent_state_independently() {
    let run = running(text_spec());
    assert_illegal(reduce(&run, bound_event("source", "watt_missing")));

    let dispatched = dispatch(&run, "source", "watt_source");

    let mut already_bound = dispatched.clone();
    already_bound.attempts[0].agent = Some(QualifiedAddress::local("existing-worker"));
    assert_illegal(reduce(&already_bound, bound_event("source", "watt_source")));

    let mut wrong_attempt_state = dispatched.clone();
    wrong_attempt_state.attempts[0].state = WorkflowAttemptState::Running;
    assert_illegal(reduce(
        &wrong_attempt_state,
        bound_event("source", "watt_source"),
    ));

    let mut wrong_node_link = dispatched.clone();
    wrong_node_link.attempts[0].node_id = WorkflowNodeId::new("output");
    assert_eq!(
        reduce(&wrong_node_link, bound_event("source", "watt_source")),
        Err(WorkflowReduceError::AttemptMismatch)
    );

    let cancelling = apply(
        &dispatched,
        WorkflowEventPayload::CancelRequested {
            reason: Some("bind race".into()),
        },
    );
    let WorkflowReduceOutcome::Applied(bound) =
        reduce(&cancelling, bound_event("source", "watt_source")).unwrap()
    else {
        panic!("fresh cancelling bind was ignored")
    };
    assert!(bound.attempts[0].agent.is_some());
}
