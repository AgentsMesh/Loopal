use loopal_protocol::*;

use crate::workflow_support::text_spec;

#[test]
fn planner_wire_is_strict_and_provider_neutral() {
    let direct =
        parse_workflow_plan(r#"{"version":1,"execution":{"kind":"direct","reason":"small"}}"#)
            .unwrap();
    assert!(matches!(
        direct.execution,
        WorkflowExecution::Direct { reason: Some(reason) } if reason == "small"
    ));

    let mut value = serde_json::json!({
        "version": 1,
        "execution": {"kind": "direct", "provider": "vendor-extension"}
    });
    assert!(parse_workflow_plan(&value.to_string()).is_err());
    value["execution"] = serde_json::json!({"kind": "direct"});
    value["version"] = serde_json::json!(2);
    assert!(matches!(
        parse_workflow_plan(&value.to_string()),
        Err(PlannerParseError::UnsupportedVersion(2))
    ));
}

#[test]
fn malformed_or_fenced_planner_output_is_rejected() {
    assert!(matches!(
        parse_workflow_plan("```json\n{}\n```"),
        Err(PlannerParseError::InvalidJson)
    ));
    assert!(matches!(
        parse_workflow_plan(&"x".repeat(MAX_WORKFLOW_PLAN_BYTES + 1)),
        Err(PlannerParseError::TooLarge)
    ));
}

#[test]
fn planner_caps_limits_but_rejects_graphs_over_the_ceiling() {
    let mut spec = text_spec();
    spec.limits.max_nodes = 64;
    spec.limits.max_parallel = 16;
    spec.limits.max_attempts = 64;
    spec.limits.run_deadline_ms = 90_000;
    spec.limits.attempt_timeout_ms = 45_000;
    spec.limits.max_output_bytes = 4_096;
    let ceilings = WorkflowPlannerCeilings {
        max_nodes: 2,
        max_parallel: 1,
        max_attempts: 2,
        max_output_bytes: 1_024,
        run_deadline_ms: 60_000,
        attempt_timeout_ms: 30_000,
    };
    let capped = cap_and_validate_workflow(spec.clone(), ceilings).unwrap();
    assert_eq!(capped.limits.max_nodes, 2);
    assert_eq!(capped.limits.max_parallel, 1);
    assert_eq!(capped.limits.max_attempts, 2);
    assert_eq!(capped.limits.run_deadline_ms, 60_000);
    assert_eq!(capped.limits.attempt_timeout_ms, 30_000);
    assert_eq!(capped.limits.max_output_bytes, 1_024);
    assert_eq!(capped.output_contract.max_bytes(), 1_024);

    let mut too_many = capped;
    too_many.nodes.push(too_many.nodes[0].clone());
    assert!(matches!(
        cap_and_validate_workflow(too_many, ceilings),
        Err(PlannerLimitError::ExceedsCeiling("max_nodes"))
    ));
}

#[test]
fn simple_goals_use_direct_policy_only_when_unambiguously_small() {
    assert!(is_deterministically_simple_goal("Fix the typo in README"));
    assert!(!is_deterministically_simple_goal(
        "\u{8bf7}\u{8ba9}\u{591a}\u{4e2a}\u{667a}\u{80fd}\u{4f53}\u{5206}\u{522b}\u{5ba1}\u{67e5}\u{5e76}\u{4ea4}\u{53c9}\u{9a8c}\u{8bc1}\u{8fd9}\u{4e2a}\u{4fee}\u{590d}"
    ));
    assert!(!is_deterministically_simple_goal(
        "Inspect the repository and ask several agents to independently cross-check it"
    ));
    assert!(!is_deterministically_simple_goal(
        &"x".repeat(SIMPLE_GOAL_MAX_BYTES + 1)
    ));
}

fn valid_ceilings() -> WorkflowPlannerCeilings {
    WorkflowPlannerCeilings {
        max_nodes: 8,
        max_parallel: 2,
        max_attempts: 8,
        max_output_bytes: 4_096,
        run_deadline_ms: 60_000,
        attempt_timeout_ms: 30_000,
    }
}

#[test]
fn planner_errors_have_stable_display_messages() {
    let parse_errors = [
        (
            PlannerParseError::Empty,
            "planner response is empty".to_string(),
        ),
        (
            PlannerParseError::TooLarge,
            "planner response exceeds the byte limit".to_string(),
        ),
        (
            PlannerParseError::InvalidJson,
            "planner response is not valid JSON".to_string(),
        ),
        (
            PlannerParseError::InvalidShape,
            "planner response has an invalid shape".to_string(),
        ),
        (
            PlannerParseError::UnsupportedVersion(7),
            "unsupported planner response version 7".to_string(),
        ),
    ];
    for (error, expected) in parse_errors {
        assert_eq!(error.to_string(), expected);
    }
    assert_eq!(parse_workflow_plan(" \n\t"), Err(PlannerParseError::Empty));

    let validation_error = validate_workflow_spec(&{
        let mut spec = text_spec();
        spec.version = 0;
        spec
    })
    .unwrap_err();
    let limit_errors = [
        (
            PlannerLimitError::InvalidCeiling("max_nodes"),
            "invalid trusted ceiling max_nodes".to_string(),
        ),
        (
            PlannerLimitError::ExceedsCeiling("max_nodes"),
            "planner workflow exceeds ceiling max_nodes".to_string(),
        ),
        (
            PlannerLimitError::InvalidSpec(validation_error.clone()),
            format!("planner workflow is invalid: {validation_error:?}"),
        ),
    ];
    for (error, expected) in limit_errors {
        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn every_planner_ceiling_fails_closed_at_zero_and_above_its_bound() {
    let mut cases = Vec::new();
    for value in [0, MAX_WORKFLOW_NODES + 1] {
        let mut ceilings = valid_ceilings();
        ceilings.max_nodes = value;
        cases.push((ceilings, "max_nodes"));
    }
    for value in [0, MAX_WORKFLOW_PARALLELISM + 1] {
        let mut ceilings = valid_ceilings();
        ceilings.max_parallel = value;
        cases.push((ceilings, "max_parallel"));
    }
    for value in [0, MAX_WORKFLOW_ATTEMPTS + 1] {
        let mut ceilings = valid_ceilings();
        ceilings.max_attempts = value;
        cases.push((ceilings, "max_attempts"));
    }
    for value in [0, MAX_WORKFLOW_OUTPUT_BYTES + 1] {
        let mut ceilings = valid_ceilings();
        ceilings.max_output_bytes = value;
        cases.push((ceilings, "max_output_bytes"));
    }
    for value in [0, MAX_WORKFLOW_RUN_DEADLINE_MS + 1] {
        let mut ceilings = valid_ceilings();
        ceilings.run_deadline_ms = value;
        cases.push((ceilings, "run_deadline_ms"));
    }
    for value in [0, MAX_WORKFLOW_ATTEMPT_TIMEOUT_MS + 1] {
        let mut ceilings = valid_ceilings();
        ceilings.attempt_timeout_ms = value;
        cases.push((ceilings, "attempt_timeout_ms"));
    }
    let mut longer_than_run = valid_ceilings();
    longer_than_run.attempt_timeout_ms = longer_than_run.run_deadline_ms + 1;
    cases.push((longer_than_run, "attempt_timeout_ms"));

    for (ceilings, field) in cases {
        assert_eq!(
            ceilings.validate(),
            Err(PlannerLimitError::InvalidCeiling(field)),
            "{field}: {ceilings:?}"
        );
    }
    valid_ceilings().validate().unwrap();
}
