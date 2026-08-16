use loopal_protocol::*;

use crate::workflow_support::*;

fn fail(
    run: &WorkflowRunSnapshot,
    attempt: &str,
    class: WorkflowFailureClass,
) -> Result<WorkflowReduceOutcome, WorkflowReduceError> {
    reduce_workflow_event(
        run,
        &event(
            run,
            WorkflowEventPayload::AttemptFailed {
                node_id: "source".into(),
                attempt_id: attempt.into(),
                completion: AgentCompletion::new("error", Some("failed".into())),
                failure: WorkflowAttemptFailure {
                    class,
                    reason: "failed".into(),
                },
            },
        ),
        &AcceptJson,
    )
}

#[test]
fn transient_pre_execution_failure_is_automatically_retryable() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_one");
    let WorkflowReduceOutcome::Applied(run) = fail(
        &run,
        "watt_one",
        WorkflowFailureClass::TransientBeforeExecution,
    )
    .unwrap() else {
        panic!("failure ignored")
    };
    let run = *run;
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Ready);
    let attempt = &run.attempts[0];
    assert_eq!(
        classify_workflow_retry(attempt, attempt.failure.as_ref().unwrap(), 1, 8),
        WorkflowRetryDisposition::Automatic
    );
    assert_eq!(run.state, WorkflowRunState::Running);
}

#[test]
fn running_attempt_downgrades_transient_claim_to_ambiguous() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_one");
    let run = bind_and_run(&run, "source", "watt_one");
    let WorkflowReduceOutcome::Applied(run) = fail(
        &run,
        "watt_one",
        WorkflowFailureClass::TransientBeforeExecution,
    )
    .unwrap() else {
        panic!("failure ignored")
    };
    let run = *run;
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Failed);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Skipped);
    assert_eq!(run.state, WorkflowRunState::Failed);
    let failure = run.attempts[0].failure.as_ref().unwrap();
    assert_eq!(failure.class, WorkflowFailureClass::AmbiguousExecution);
    assert_eq!(
        classify_workflow_retry(&run.attempts[0], failure, 1, 8),
        WorkflowRetryDisposition::ExplicitOnly
    );
}

#[test]
fn permanent_failure_is_never_retryable() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_one");
    let WorkflowReduceOutcome::Applied(run) =
        fail(&run, "watt_one", WorkflowFailureClass::Permanent).unwrap()
    else {
        panic!("failure ignored")
    };
    let run = *run;
    assert_eq!(run.state, WorkflowRunState::Failed);
    let failure = run.attempts[0].failure.as_ref().unwrap();
    assert_eq!(
        classify_workflow_retry(&run.attempts[0], failure, 1, 8),
        WorkflowRetryDisposition::Never
    );
}

#[test]
fn text_result_enforces_type_and_byte_bound() {
    let contract = WorkflowOutputContract::Text { max_bytes: 4 };
    validate_workflow_output(&contract, &WorkflowOutput::Text("four".into()), &AcceptJson).unwrap();
    assert!(matches!(
        validate_workflow_output(
            &contract,
            &WorkflowOutput::Text("large".into()),
            &AcceptJson,
        ),
        Err(WorkflowOutputValidationError::TooLarge { .. })
    ));
    assert!(matches!(
        validate_workflow_output(
            &contract,
            &WorkflowOutput::Json(serde_json::json!({})),
            &AcceptJson,
        ),
        Err(WorkflowOutputValidationError::ContractMismatch { .. })
    ));
}

#[test]
fn json_result_requires_external_semantic_validator() {
    let contract = WorkflowOutputContract::Json {
        max_bytes: 64,
        schema: serde_json::json!({"type": "object"}),
    };
    let value = WorkflowOutput::Json(serde_json::json!({"answer": 42}));
    validate_workflow_output(&contract, &value, &AcceptJson).unwrap();
    assert_eq!(
        validate_workflow_output(&contract, &value, &RejectJson),
        Err(WorkflowOutputValidationError::SchemaViolation {
            detail: "schema mismatch".into()
        })
    );
}

#[test]
fn output_node_cannot_succeed_before_result_validation() {
    let run = running(json_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let run = succeed(&run, "source", "watt_source", None);
    let run = dispatch(&run, "output", "watt_output");
    let run = bind_and_run(&run, "output", "watt_output");
    let error = reduce_workflow_event(
        &run,
        &event(
            &run,
            WorkflowEventPayload::AttemptSucceeded {
                node_id: "output".into(),
                attempt_id: "watt_output".into(),
                completion: AgentCompletion::goal(Some("done".into())),
                output: Some(WorkflowOutput::Json(serde_json::json!({"answer": 42}))),
            },
        ),
        &RejectJson,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkflowReduceError::OutputValidation { .. }
    ));
    assert_eq!(run.state, WorkflowRunState::Running);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Running);
}
