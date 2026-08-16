use loopal_protocol::*;

use crate::workflow_support::*;

#[test]
fn reducer_requires_internal_spec_validation_before_start() {
    let run = planned(text_spec());
    let error = reduce_workflow_event(
        &run,
        &event(&run, WorkflowEventPayload::RunStarted),
        &AcceptJson,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        WorkflowReduceError::IllegalTransition { .. }
    ));

    let mut invalid = text_spec();
    invalid.nodes[0].id = "../bad".into();
    let run = planned(invalid);
    let error = reduce_workflow_event(
        &run,
        &event(&run, WorkflowEventPayload::SpecValidated),
        &AcceptJson,
    )
    .unwrap_err();
    assert!(matches!(error, WorkflowReduceError::Validation { .. }));
    assert_eq!(run.state, WorkflowRunState::Planned);
}

#[test]
fn all_success_dependencies_release_and_output_commits_last() {
    let run = running(text_spec());
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Ready);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Pending);

    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let run = succeed(&run, "source", "watt_source", None);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Ready);
    assert_eq!(run.state, WorkflowRunState::Running);

    let run = dispatch(&run, "output", "watt_output");
    let run = bind_and_run(&run, "output", "watt_output");
    let run = succeed(
        &run,
        "output",
        "watt_output",
        Some(WorkflowOutput::Text("answer".into())),
    );
    assert_eq!(run.state, WorkflowRunState::Succeeded);
    assert_eq!(run.result, Some(WorkflowOutput::Text("answer".into())));
}

#[test]
fn stale_events_are_ignored_and_revision_gaps_rejected() {
    let run = running(text_spec());
    let stale = WorkflowEvent {
        run_id: run.id.clone(),
        revision: run.revision,
        occurred_at_unix_ms: 999,
        payload: WorkflowEventPayload::CancelRequested { reason: None },
    };
    assert_eq!(
        reduce_workflow_event(&run, &stale, &AcceptJson).unwrap(),
        WorkflowReduceOutcome::IgnoredStale {
            current_revision: run.revision
        }
    );

    let gap = WorkflowEvent {
        revision: run.revision + 2,
        ..stale
    };
    assert!(matches!(
        reduce_workflow_event(&run, &gap, &AcceptJson),
        Err(WorkflowReduceError::RevisionGap { .. })
    ));
}

#[test]
fn duplicate_attempt_events_do_not_apply_twice() {
    let run = running(text_spec());
    let dispatch_event = event(
        &run,
        WorkflowEventPayload::DispatchIntended {
            node_id: "source".into(),
            attempt_id: "watt_one".into(),
            capability_digest: WorkflowAttemptCapability::parse("44".repeat(32))
                .unwrap()
                .digest(),
        },
    );
    let WorkflowReduceOutcome::Applied(next) =
        reduce_workflow_event(&run, &dispatch_event, &AcceptJson).unwrap()
    else {
        panic!("dispatch ignored")
    };
    let next = *next;
    assert!(matches!(
        reduce_workflow_event(&next, &dispatch_event, &AcceptJson).unwrap(),
        WorkflowReduceOutcome::IgnoredStale { .. }
    ));
    assert_eq!(next.attempts.len(), 1);
    assert_eq!(next.nodes[0].attempt_count, 1);
}

#[test]
fn event_identity_mismatches_fail_closed() {
    let run = running(text_spec());
    let wrong_run = WorkflowEvent {
        run_id: "wrun_other".into(),
        ..event(&run, WorkflowEventPayload::CancelRequested { reason: None })
    };
    assert_eq!(
        reduce_workflow_event(&run, &wrong_run, &AcceptJson),
        Err(WorkflowReduceError::WrongRun)
    );

    let run = dispatch(&run, "source", "watt_one");
    let error = reduce_workflow_event(
        &run,
        &event(
            &run,
            WorkflowEventPayload::AttemptBound {
                node_id: "source".into(),
                attempt_id: "watt_other".into(),
                agent: QualifiedAddress::local("worker"),
            },
        ),
        &AcceptJson,
    )
    .unwrap_err();
    assert!(matches!(error, WorkflowReduceError::AttemptMismatch));
}

#[test]
fn cancellation_stops_admission_and_becomes_terminal_after_active_attempts() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let run = apply(
        &run,
        WorkflowEventPayload::CancelRequested {
            reason: Some("user".into()),
        },
    );
    assert_eq!(run.state, WorkflowRunState::Cancelling);
    assert_eq!(run.nodes[0].state, WorkflowNodeState::Cancelling);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Cancelled);
    assert!(matches!(
        reduce_workflow_event(
            &run,
            &event(
                &run,
                WorkflowEventPayload::DispatchIntended {
                    node_id: "output".into(),
                    attempt_id: "watt_late".into(),
                    capability_digest: WorkflowAttemptCapability::parse("55".repeat(32))
                        .unwrap()
                        .digest(),
                },
            ),
            &AcceptJson,
        ),
        Err(WorkflowReduceError::IllegalTransition { .. })
    ));
    let run = apply(
        &run,
        WorkflowEventPayload::AttemptCancelled {
            node_id: "source".into(),
            attempt_id: "watt_source".into(),
            reason: "interrupted".into(),
        },
    );
    assert_eq!(run.state, WorkflowRunState::Cancelled);
}

#[test]
fn terminal_runs_are_immutable_even_to_next_revision() {
    let run = running(text_spec());
    let run = apply(&run, WorkflowEventPayload::CancelRequested { reason: None });
    assert_eq!(run.state, WorkflowRunState::Cancelled);
    assert_eq!(
        reduce_workflow_event(
            &run,
            &event(&run, WorkflowEventPayload::CancelRequested { reason: None }),
            &AcceptJson,
        ),
        Err(WorkflowReduceError::TerminalImmutable)
    );
}
