use crate::workflow_support::*;
use loopal_protocol::*;
type SpecMutation = Box<dyn FnOnce(&mut WorkflowSpec)>;
type ErrorPredicate = fn(&WorkflowValidationError) -> bool;

fn assert_error(
    mutate: impl FnOnce(&mut WorkflowSpec),
    expected: impl FnOnce(&WorkflowValidationError) -> bool,
) {
    let mut spec = text_spec();
    mutate(&mut spec);
    let error = validate_workflow_spec(&spec).unwrap_err();
    assert!(expected(&error), "unexpected error: {error:?}");
}

#[test]
fn valid_text_and_json_dags_pass() {
    validate_workflow_spec(&text_spec()).unwrap();
    validate_workflow_spec(&json_spec()).unwrap();
}

#[test]
fn graph_shape_errors_are_rejected() {
    let cases: Vec<(SpecMutation, ErrorPredicate)> = vec![
        (Box::new(|s| s.nodes.push(node("source", &[]))), |e| {
            matches!(e, WorkflowValidationError::DuplicateNodeId { .. })
        }),
        (
            Box::new(|s| s.nodes[1].dependencies = vec!["missing".into()]),
            |e| matches!(e, WorkflowValidationError::MissingDependency { .. }),
        ),
        (
            Box::new(|s| s.nodes[0].dependencies = vec!["source".into()]),
            |e| matches!(e, WorkflowValidationError::SelfDependency { .. }),
        ),
        (
            Box::new(|s| s.nodes[1].dependencies.push("source".into())),
            |e| matches!(e, WorkflowValidationError::DuplicateDependency { .. }),
        ),
        (
            Box::new(|s| s.nodes[0].dependencies = vec!["output".into()]),
            |e| matches!(e, WorkflowValidationError::Cycle),
        ),
        (Box::new(|s| s.output_node = "missing".into()), |e| {
            matches!(e, WorkflowValidationError::MissingOutputNode { .. })
        }),
    ];
    for (mutate, expected) in cases {
        assert_error(mutate, expected);
    }
}

#[test]
fn empty_oversized_and_path_like_ids_are_rejected() {
    for invalid in [
        String::new(),
        "../escape".into(),
        "profile.name".into(),
        "x".repeat(MAX_WORKFLOW_ID_BYTES + 1),
    ] {
        assert_error(
            |spec| spec.nodes[0].id = WorkflowNodeId::new(invalid),
            |error| matches!(error, WorkflowValidationError::InvalidNodeId { .. }),
        );
    }
    for invalid in [
        String::new(),
        "../admin".into(),
        "x".repeat(MAX_WORKER_PROFILE_BYTES + 1),
    ] {
        assert_error(
            |spec| spec.nodes[0].worker_profile = WorkflowWorkerProfileRef::new(invalid),
            |error| matches!(error, WorkflowValidationError::InvalidWorkerProfile { .. }),
        );
    }
}

#[test]
fn text_and_goal_bounds_are_rejected() {
    assert_error(
        |spec| spec.run_goal.clear(),
        |error| matches!(error, WorkflowValidationError::EmptyGoal),
    );
    assert_error(
        |spec| spec.run_goal = "x".repeat(MAX_WORKFLOW_GOAL_BYTES + 1),
        |error| matches!(error, WorkflowValidationError::GoalTooLong),
    );
    assert_error(
        |spec| spec.nodes[0].task.clear(),
        |error| matches!(error, WorkflowValidationError::EmptyTask { .. }),
    );
    assert_error(
        |spec| spec.nodes[0].task = "x".repeat(MAX_WORKFLOW_TASK_BYTES + 1),
        |error| matches!(error, WorkflowValidationError::TaskTooLong { .. }),
    );
}

#[test]
fn every_limit_is_bounded_and_consistent() {
    let cases: Vec<(SpecMutation, &str)> = vec![
        (
            Box::new(|s| s.limits.max_nodes = MAX_WORKFLOW_NODES + 1),
            "max_nodes",
        ),
        (
            Box::new(|s| s.limits.max_parallel = MAX_WORKFLOW_PARALLELISM + 1),
            "max_parallel",
        ),
        (
            Box::new(|s| s.limits.max_attempts = MAX_WORKFLOW_ATTEMPTS + 1),
            "max_attempts",
        ),
        (
            Box::new(|s| s.limits.run_deadline_ms = MAX_WORKFLOW_RUN_DEADLINE_MS + 1),
            "run_deadline_ms",
        ),
        (
            Box::new(|s| s.limits.attempt_timeout_ms = MAX_WORKFLOW_ATTEMPT_TIMEOUT_MS + 1),
            "attempt_timeout_ms",
        ),
        (
            Box::new(|s| s.limits.max_output_bytes = MAX_WORKFLOW_OUTPUT_BYTES + 1),
            "max_output_bytes",
        ),
    ];
    for (mutate, field) in cases {
        assert_error(
            mutate,
            |error| matches!(error, WorkflowValidationError::InvalidLimit { field: actual } if actual == field),
        );
    }
    assert_error(
        |spec| {
            spec.limits.max_nodes = 1;
            spec.limits.max_parallel = 1;
        },
        |error| matches!(error, WorkflowValidationError::NodeLimit { .. }),
    );
    assert_error(
        |spec| spec.output_contract = WorkflowOutputContract::Text { max_bytes: 4_097 },
        |error| matches!(error, WorkflowValidationError::InvalidOutputBound),
    );
}

#[test]
fn aggregate_spec_size_is_bounded_for_start_admission() {
    assert_error(
        |spec| {
            let task = "x".repeat(MAX_WORKFLOW_TASK_BYTES);
            spec.nodes = (0..32)
                .map(|index| WorkflowAgentNode {
                    id: WorkflowNodeId::new(format!("node_{index}")),
                    dependencies: Vec::new(),
                    task: task.clone(),
                    worker_profile: WorkflowWorkerProfileRef::new("default"),
                })
                .collect();
            spec.output_node = "node_0".into();
            spec.limits.max_nodes = 32;
            spec.limits.max_parallel = 32;
            spec.limits.max_attempts = 32;
        },
        |error| matches!(error, WorkflowValidationError::SpecTooLarge),
    );
}

#[test]
fn schema_structure_is_bounded_without_claiming_semantic_validation() {
    assert_error(
        |spec| {
            spec.output_contract = WorkflowOutputContract::Json {
                max_bytes: 100,
                schema: serde_json::json!(true),
            }
        },
        |error| matches!(error, WorkflowValidationError::SchemaRootNotObject),
    );
    assert_error(
        |spec| {
            let mut value = serde_json::json!({});
            for _ in 0..=MAX_JSON_SCHEMA_DEPTH {
                value = serde_json::json!({"nested": value});
            }
            spec.output_contract = WorkflowOutputContract::Json {
                max_bytes: 100,
                schema: value,
            };
        },
        |error| matches!(error, WorkflowValidationError::SchemaTooDeep),
    );
    assert_error(
        |spec| {
            spec.output_contract = WorkflowOutputContract::Json {
                max_bytes: 100,
                schema: serde_json::json!({"description": "x".repeat(MAX_JSON_SCHEMA_STRING_BYTES + 1)}),
            }
        },
        |error| matches!(error, WorkflowValidationError::SchemaStringTooLong),
    );
}
