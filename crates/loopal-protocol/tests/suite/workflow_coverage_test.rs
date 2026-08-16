use loopal_protocol::*;

use crate::workflow_support::*;

#[test]
fn all_id_conversion_traits_and_profile_display_are_exercised() {
    let run = WorkflowRunId::from(String::from("wrun_string"));
    let node = WorkflowNodeId::from(String::from("node_string"));
    let attempt = WorkflowAttemptId::from(String::from("watt_string"));
    let request = WorkflowRequestId::from(String::from("wreq_string"));
    for value in [
        run.as_ref(),
        node.as_ref(),
        attempt.as_ref(),
        request.as_ref(),
    ] {
        assert!(!value.is_empty());
    }
    let profile = WorkflowWorkerProfileRef::new("profile");
    assert_eq!(profile.as_str(), "profile");
    assert_eq!(profile.to_string(), "profile");
}

#[test]
fn extra_graph_validation_bounds_are_rejected() {
    let mut spec = text_spec();
    spec.nodes[0].dependencies = (0..=MAX_DEPENDENCIES_PER_NODE)
        .map(|index| WorkflowNodeId::new(format!("dep_{index}")))
        .collect();
    assert!(matches!(
        validate_workflow_spec(&spec),
        Err(WorkflowValidationError::TooManyDependencies { .. })
    ));

    let mut spec = text_spec();
    spec.nodes[1].dependencies = vec!["../bad".into()];
    assert!(matches!(
        validate_workflow_spec(&spec),
        Err(WorkflowValidationError::InvalidNodeId { .. })
    ));
}

#[test]
fn additional_schema_bounds_are_rejected() {
    let cases = [
        serde_json::json!({"x".repeat(MAX_JSON_SCHEMA_KEY_BYTES + 1): true}),
        serde_json::Value::Object(
            (0..MAX_JSON_SCHEMA_NODES)
                .map(|index| (format!("k{index}"), serde_json::json!(true)))
                .collect(),
        ),
        serde_json::json!({"description": "x".repeat(MAX_JSON_SCHEMA_BYTES)}),
    ];
    let expected = [
        WorkflowValidationError::SchemaKeyTooLong,
        WorkflowValidationError::SchemaTooComplex,
        WorkflowValidationError::SchemaTooLarge,
    ];
    for (schema, expected) in cases.into_iter().zip(expected) {
        let mut spec = json_spec();
        spec.output_contract = WorkflowOutputContract::Json {
            max_bytes: 100,
            schema,
        };
        assert_eq!(validate_workflow_spec(&spec), Err(expected));
    }
}

#[test]
fn reducer_rejects_invalid_identity_and_illegal_attempt_transitions() {
    let invalid = WorkflowRunSnapshot::planned(
        WorkflowRunId::new("../bad"),
        QualifiedAddress::local("root"),
        text_spec(),
        1,
    );
    assert_eq!(
        reduce_workflow_event(
            &invalid,
            &event(&invalid, WorkflowEventPayload::SpecValidated),
            &AcceptJson,
        ),
        Err(WorkflowReduceError::InvalidRunId)
    );

    let run = running(text_spec());
    assert_eq!(
        reduce_workflow_event(
            &run,
            &event(
                &run,
                WorkflowEventPayload::DispatchIntended {
                    node_id: "source".into(),
                    attempt_id: "../bad".into(),
                    capability_digest: WorkflowAttemptCapability::parse("66".repeat(32))
                        .unwrap()
                        .digest(),
                },
            ),
            &AcceptJson,
        ),
        Err(WorkflowReduceError::InvalidAttemptId)
    );
}

#[test]
fn cancel_from_planned_and_validated_finishes_immediately() {
    for run in [planned(text_spec()), {
        let run = planned(text_spec());
        apply(&run, WorkflowEventPayload::SpecValidated)
    }] {
        let run = apply(&run, WorkflowEventPayload::CancelRequested { reason: None });
        assert_eq!(run.state, WorkflowRunState::Cancelled);
        assert!(
            run.nodes
                .iter()
                .all(|node| node.state == WorkflowNodeState::Cancelled)
        );
    }
}

#[test]
fn summary_counts_every_node_state_bucket() {
    let mut spec = text_spec();
    spec.nodes = [
        "pending",
        "ready",
        "dispatching",
        "running",
        "cancelling",
        "succeeded",
        "failed",
        "cancelled",
        "skipped",
    ]
    .into_iter()
    .map(|id| node(id, &[]))
    .collect();
    spec.output_node = "succeeded".into();
    spec.limits.max_nodes = 9;
    spec.limits.max_parallel = 9;
    spec.limits.max_attempts = 9;
    let mut run = planned(spec);
    let states = [
        WorkflowNodeState::Pending,
        WorkflowNodeState::Ready,
        WorkflowNodeState::Dispatching,
        WorkflowNodeState::Running,
        WorkflowNodeState::Cancelling,
        WorkflowNodeState::Succeeded,
        WorkflowNodeState::Failed,
        WorkflowNodeState::Cancelled,
        WorkflowNodeState::Skipped,
    ];
    for (node, state) in run.nodes.iter_mut().zip(states) {
        node.state = state;
    }
    let counts = WorkflowRunSummary::from(&run).counts;
    assert_eq!((counts.pending, counts.ready, counts.active), (1, 1, 3));
    assert_eq!(
        (
            counts.succeeded,
            counts.failed,
            counts.cancelled,
            counts.skipped
        ),
        (1, 1, 1, 1)
    );
}
