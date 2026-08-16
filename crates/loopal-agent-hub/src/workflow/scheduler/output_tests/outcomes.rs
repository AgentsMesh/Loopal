#[test]
fn resolved_secret_is_redacted_before_workflow_outcome_event() {
    let seed = FinalSinkRedactionSeed::new();
    seed.observe("token", "workflow-secret".into()).unwrap();
    let run = WorkflowRunSnapshot::planned(
        WorkflowRunId::new("wrun_redaction"),
        QualifiedAddress::local("root"),
        spec(),
        1,
    );
    let key = AttemptKey {
        run_id: run.id.clone(),
        node_id: WorkflowNodeId::new("source"),
        attempt_id: WorkflowAttemptId::new("watt_redaction"),
    };

    let prepared = prepare_outcome(
        &run,
        &key,
        WorkflowWorkerOutcome::Succeeded {
            completion: AgentCompletion::goal(Some("result=workflow-secret".into())),
            output: Some(WorkflowOutput::Text("output=workflow-secret".into())),
        },
        &seed,
    );

    let encoded = serde_json::to_string(&prepared.payload).unwrap();
    assert!(matches!(
        prepared.payload,
        WorkflowEventPayload::AttemptSucceeded { .. }
    ));
    assert!(encoded.contains("<secret_ref:token>"));
    assert!(!encoded.contains("workflow-secret"));
}

#[test]
fn invalid_success_and_failure_shapes_are_rejected() {
    let run = run("wrun_rejected");
    let key = key(&run, "watt_rejected");
    let seed = FinalSinkRedactionSeed::new();

    for outcome in [
        WorkflowWorkerOutcome::Succeeded {
            completion: AgentCompletion::new("cancelled", None),
            output: None,
        },
        WorkflowWorkerOutcome::Failed(WorkflowSpawnFailure {
            completion: AgentCompletion::goal(Some("not a failure".into())),
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::Permanent,
                reason: "invalid failure".into(),
            },
        }),
    ] {
        assert_rejected(prepare_outcome(&run, &key, outcome, &seed));
    }

    let mut limited = run.clone();
    limited.spec.limits.max_output_bytes = 0;
    assert_rejected(prepare_outcome(
        &limited,
        &key,
        WorkflowWorkerOutcome::Succeeded {
            completion: AgentCompletion::goal(Some("too large".into())),
            output: None,
        },
        &seed,
    ));
}

#[test]
fn aliased_completion_and_typed_output_share_one_byte_allowance() {
    let mut run = run("wrun_shared_allowance");
    run.spec.limits.max_output_bytes = 5;
    let key = key(&run, "watt_shared_allowance");

    let prepared = prepare_outcome(
        &run,
        &key,
        WorkflowWorkerOutcome::Succeeded {
            completion: AgentCompletion::goal(Some("12345".into())),
            output: Some(WorkflowOutput::Text("12345".into())),
        },
        &FinalSinkRedactionSeed::new(),
    );

    assert!(matches!(
        prepared.payload,
        WorkflowEventPayload::AttemptSucceeded {
            completion: AgentCompletion { result: Some(ref result), .. },
            output: Some(WorkflowOutput::Text(ref output)),
            ..
        } if result == "12345" && output == "12345"
    ));
}

#[test]
fn failure_none_and_json_outcomes_preserve_their_typed_shape() {
    let run = run("wrun_shapes");
    let key = key(&run, "watt_shapes");
    let seed = FinalSinkRedactionSeed::new();
    let failure = prepare_outcome(
        &run,
        &key,
        WorkflowWorkerOutcome::Failed(WorkflowSpawnFailure {
            completion: AgentCompletion::new("worker_failed", Some("details".into())),
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::TransientBeforeExecution,
                reason: "retry later".into(),
            },
        }),
        &seed,
    );
    assert!(matches!(
        failure.payload,
        WorkflowEventPayload::AttemptFailed { .. }
    ));

    for output in [
        None,
        Some(WorkflowOutput::Json(serde_json::json!({"ok": true}))),
    ] {
        let success = prepare_outcome(
            &run,
            &key,
            WorkflowWorkerOutcome::Succeeded {
                completion: AgentCompletion::goal(None),
                output: output.clone(),
            },
            &seed,
        );
        assert!(matches!(
            success.payload,
            WorkflowEventPayload::AttemptSucceeded {
                output: actual,
                ..
            } if actual == output
        ));
    }
}
