use std::str::FromStr;

use loopal_protocol::*;
use serde_json::Value;

use crate::workflow_support::*;

fn digest(byte: u8) -> PermissionActionDigest {
    PermissionActionDigest::from_bytes([byte; 32])
}

fn seed(workflow: Option<WorkflowPermissionCausation>) -> PermissionIntentSeed {
    PermissionIntentSeed::new(
        "Bash",
        digest(1),
        PermissionDisplayDigest::from_bytes([2; 32]),
        PermissionSchemaDigest::from_bytes([3; 32]),
        workflow,
    )
    .unwrap()
}

#[test]
fn permission_compound_guards_reject_each_independent_boundary() {
    let wrong_prefix = "x".repeat("sha256:".len() + 64);
    assert!(PermissionActionDigest::from_str(&wrong_prefix).is_err());

    for (node_id, attempt_id) in [("../bad", "watt_ok"), ("wnode_ok", "../bad")] {
        let workflow = WorkflowPermissionCausation {
            run_id: WorkflowRunId::new("wrun_ok"),
            node_id: WorkflowNodeId::new(node_id),
            attempt_id: WorkflowAttemptId::new(attempt_id),
        };
        assert!(
            PermissionIntentSeed::new(
                "Bash",
                digest(1),
                PermissionDisplayDigest::from_bytes([2; 32]),
                PermissionSchemaDigest::from_bytes([3; 32]),
                Some(workflow),
            )
            .is_err()
        );
    }

    assert!(
        PermissionIntentSeed::new(
            "x".repeat(257),
            digest(1),
            PermissionDisplayDigest::from_bytes([2; 32]),
            PermissionSchemaDigest::from_bytes([3; 32]),
            None,
        )
        .is_err()
    );
    assert!(PermissionIntent::bind(seed(None), 1, 1, "x".repeat(129)).is_err());
    assert!(PermissionIntent::bind(seed(None), 1, 1, "bad\ntoken").is_err());

    assert_eq!(
        PermissionIntentRequest::create(
            "x".repeat(257),
            "Bash",
            Value::Null,
            Value::Null,
            Value::Null,
            None,
        ),
        Err(PermissionRequestError::ToolCallId)
    );
}

#[test]
fn direct_policy_checks_empty_and_multiline_goals() {
    assert!(!is_deterministically_simple_goal("   \n"));
    assert!(!is_deterministically_simple_goal(
        "one\ntwo\nthree\nfour\nfive"
    ));
}

#[test]
fn reducer_checks_invalid_event_identity_and_node_state_separately() {
    let run = running(text_spec());
    let mut invalid_event = event(&run, WorkflowEventPayload::CancelRequested { reason: None });
    invalid_event.run_id = WorkflowRunId::new("../bad");
    assert_eq!(
        reduce_workflow_event(&run, &invalid_event, &AcceptJson),
        Err(WorkflowReduceError::InvalidRunId)
    );

    let mut dispatched = dispatch(&run, "source", "watt_source");
    dispatched.nodes[0].state = WorkflowNodeState::Running;
    assert!(matches!(
        reduce_workflow_event(
            &dispatched,
            &event(
                &dispatched,
                WorkflowEventPayload::AttemptRunning {
                    node_id: "source".into(),
                    attempt_id: "watt_source".into(),
                },
            ),
            &AcceptJson,
        ),
        Err(WorkflowReduceError::IllegalTransition { .. })
    ));
}

#[test]
fn release_observes_nonterminal_dependencies_without_skipping_them() {
    let mut spec = text_spec();
    spec.nodes = vec![
        node("done", &[]),
        node("blocker", &[]),
        node("output", &["blocker"]),
    ];
    spec.limits.max_nodes = 3;
    spec.output_node = "output".into();
    let run = running(spec);
    let run = dispatch(&run, "done", "watt_done");
    let run = bind_and_run(&run, "done", "watt_done");
    let run = succeed(&run, "done", "watt_done", None);
    assert_eq!(run.nodes[1].state, WorkflowNodeState::Ready);
    assert_eq!(run.nodes[2].state, WorkflowNodeState::Pending);
}

#[test]
fn terminal_result_search_skips_unrelated_attempts() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let run = succeed(&run, "source", "watt_source", None);
    let run = dispatch(&run, "output", "watt_output");
    let mut run = bind_and_run(&run, "output", "watt_output");
    let mut decoy = run.attempts[0].clone();
    decoy.id = WorkflowAttemptId::new("watt_decoy");
    run.attempts.push(decoy);
    let run = succeed(
        &run,
        "output",
        "watt_output",
        Some(WorkflowOutput::Text("done".into())),
    );
    assert_eq!(run.state, WorkflowRunState::Succeeded);
}

#[test]
fn ledger_byte_limit_precedes_record_count_limit() {
    let payload = serde_json::json!("x".repeat(MAX_WORKFLOW_REQUEST_PAYLOAD_BYTES - 2));
    let mut ledger = WorkflowRequestLedger::default();
    for index in 0..MAX_WORKFLOW_REQUEST_RECORDS {
        let result = ledger.record(WorkflowRequestRecord {
            request_id: WorkflowRequestId::new(format!("wreq_large_{index}")),
            operation: "start".into(),
            payload: payload.clone(),
            response: Value::Null,
        });
        if result == Err(WorkflowRequestError::LedgerFull) {
            assert!(ledger.records().len() < MAX_WORKFLOW_REQUEST_RECORDS);
            return;
        }
        result.unwrap();
    }
    panic!("ledger byte limit was not enforced");
}

#[test]
fn running_permanent_failure_preserves_its_class() {
    let run = running(text_spec());
    let run = dispatch(&run, "source", "watt_source");
    let run = bind_and_run(&run, "source", "watt_source");
    let event = event(
        &run,
        WorkflowEventPayload::AttemptFailed {
            node_id: "source".into(),
            attempt_id: "watt_source".into(),
            completion: AgentCompletion::new("error", None),
            failure: WorkflowAttemptFailure {
                class: WorkflowFailureClass::Permanent,
                reason: "permanent".into(),
            },
        },
    );
    let WorkflowReduceOutcome::Applied(run) =
        reduce_workflow_event(&run, &event, &AcceptJson).unwrap()
    else {
        panic!("fresh event was ignored")
    };
    assert_eq!(
        run.attempts[0].failure.as_ref().unwrap().class,
        WorkflowFailureClass::Permanent
    );
}
