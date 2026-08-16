use crate::workflow_support::{node, text_spec};
use loopal_protocol::{
    WORKFLOW_SPEC_V1, WorkflowOutputContract, WorkflowSpec, WorkflowValidationError,
    WorkflowWorkerProfileRef, validate_workflow_spec,
};

type LimitMutation = fn(&mut WorkflowSpec);

fn assert_limit(mut spec: WorkflowSpec, mutate: LimitMutation, field: &str) {
    mutate(&mut spec);
    assert!(matches!(
        validate_workflow_spec(&spec),
        Err(WorkflowValidationError::InvalidLimit { field: actual }) if actual == field
    ));
}

#[test]
fn zero_and_exhausted_limits_fail_closed() {
    let cases: [(LimitMutation, &str); 6] = [
        (|spec| spec.limits.max_nodes = 0, "max_nodes"),
        (|spec| spec.limits.max_parallel = 0, "max_parallel"),
        (|spec| spec.limits.max_attempts = 1, "max_attempts"),
        (|spec| spec.limits.run_deadline_ms = 0, "run_deadline_ms"),
        (
            |spec| spec.limits.attempt_timeout_ms = 0,
            "attempt_timeout_ms",
        ),
        (|spec| spec.limits.max_output_bytes = 0, "max_output_bytes"),
    ];

    for (mutate, field) in cases {
        assert_limit(text_spec(), mutate, field);
    }
}

#[test]
fn header_output_and_profile_boundaries_fail_closed() {
    let mut unsupported = text_spec();
    unsupported.version = WORKFLOW_SPEC_V1 + 1;
    assert!(matches!(
        validate_workflow_spec(&unsupported),
        Err(WorkflowValidationError::UnsupportedVersion { .. })
    ));

    let mut empty = text_spec();
    empty.nodes.clear();
    assert_eq!(
        validate_workflow_spec(&empty),
        Err(WorkflowValidationError::EmptyGraph)
    );

    let mut output = text_spec();
    output.output_contract = WorkflowOutputContract::Text { max_bytes: 0 };
    assert_eq!(
        validate_workflow_spec(&output),
        Err(WorkflowValidationError::InvalidOutputBound)
    );

    let mut profile = text_spec();
    profile.nodes[0].worker_profile = WorkflowWorkerProfileRef::new("bad.profile");
    assert!(matches!(
        validate_workflow_spec(&profile),
        Err(WorkflowValidationError::InvalidWorkerProfile { .. })
    ));
}

#[test]
fn multi_root_join_graph_is_acyclic() {
    let mut spec = text_spec();
    spec.nodes = vec![
        node("source_a", &[]),
        node("source_b", &[]),
        node("output", &["source_a", "source_b"]),
    ];
    spec.limits.max_attempts = 3;

    validate_workflow_spec(&spec).unwrap();
}
