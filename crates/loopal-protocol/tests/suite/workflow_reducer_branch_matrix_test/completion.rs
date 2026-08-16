use super::*;

#[test]
fn running_requires_both_dispatch_state_and_a_bound_agent() {
    let run = running(text_spec());
    let dispatched = dispatch(&run, "source", "watt_source");
    assert_illegal(reduce(&dispatched, running_event("source", "watt_source")));

    let mut wrong_state = dispatched;
    wrong_state.attempts[0].state = WorkflowAttemptState::Running;
    wrong_state.attempts[0].agent = Some(QualifiedAddress::local("worker"));
    assert_illegal(reduce(&wrong_state, running_event("source", "watt_source")));
}

#[test]
fn success_and_failure_reject_mismatched_completion_and_attempt_state() {
    let run = running(text_spec());
    let dispatched = dispatch(&run, "source", "watt_source");
    let active = bind_and_run(&dispatched, "source", "watt_source");

    assert_eq!(
        reduce(
            &active,
            WorkflowEventPayload::AttemptSucceeded {
                node_id: "source".into(),
                attempt_id: "watt_source".into(),
                completion: AgentCompletion::new("error", Some("not a goal".into())),
                output: None,
            },
        ),
        Err(WorkflowReduceError::InvalidCompletion)
    );

    let mut wrong_success_state = active;
    wrong_success_state.attempts[0].state = WorkflowAttemptState::Dispatching;
    assert_illegal(reduce(
        &wrong_success_state,
        WorkflowEventPayload::AttemptSucceeded {
            node_id: "source".into(),
            attempt_id: "watt_source".into(),
            completion: AgentCompletion::goal(Some("done".into())),
            output: None,
        },
    ));

    assert_eq!(
        reduce(
            &dispatched,
            failed_event(
                "source",
                "watt_source",
                AgentCompletion::goal(Some("unexpected success".into())),
            ),
        ),
        Err(WorkflowReduceError::InvalidCompletion)
    );

    let mut wrong_failure_state = dispatched;
    wrong_failure_state.attempts[0].state = WorkflowAttemptState::Succeeded;
    assert_illegal(reduce(
        &wrong_failure_state,
        failed_event("source", "watt_source", AgentCompletion::new("error", None)),
    ));

    let inactive = running(text_spec());
    assert_illegal(reduce(
        &inactive,
        failed_event(
            "source",
            "watt_missing",
            AgentCompletion::new("error", None),
        ),
    ));
}

#[test]
fn retry_classifier_covers_capacity_and_entered_execution_edges() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let attempt = &run.attempts[0];
    let failure = WorkflowAttemptFailure {
        class: WorkflowFailureClass::TransientBeforeExecution,
        reason: "retry edge".into(),
    };
    assert_eq!(
        classify_workflow_retry(attempt, &failure, 8, 8),
        WorkflowRetryDisposition::ExplicitOnly
    );

    let mut entered = attempt.clone();
    entered.entered_running = true;
    assert_eq!(
        classify_workflow_retry(&entered, &failure, 0, 8),
        WorkflowRetryDisposition::ExplicitOnly
    );
}

#[test]
fn request_ledger_rejects_an_impossible_response_reservation() {
    let ledger = WorkflowRequestLedger::default();
    assert_eq!(
        ledger.decide_with_response_size(
            &WorkflowRequestId::new("wreq_response_bound"),
            "start",
            &serde_json::Value::Null,
            MAX_WORKFLOW_REQUEST_RESPONSE_BYTES + 1,
        ),
        Err(WorkflowRequestError::ResponseTooLarge)
    );
}
