use loopal_config::SandboxPolicy;
use loopal_protocol::{
    WorkflowAttemptCapability, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation,
    WorkflowRunId,
};

use super::parse_start_params;

fn parse(params: serde_json::Value) -> anyhow::Result<crate::params::StartParams> {
    parse_start_params(&params).map(|(start, _, _)| start)
}

fn parse_error(params: serde_json::Value) -> String {
    match parse(params) {
        Ok(_) => panic!("expected parsing to fail"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn parses_all_lifecycle_modes_and_preserves_prompt_default() {
    for (wire, expected) in [
        ("ephemeral", loopal_runtime::LifecycleMode::Ephemeral),
        ("persistent", loopal_runtime::LifecycleMode::Persistent),
        (
            "workflow_ephemeral",
            loopal_runtime::LifecycleMode::WorkflowEphemeral,
        ),
    ] {
        let (_, _, lifecycle) =
            parse_start_params(&serde_json::json!({ "lifecycle": wire })).unwrap();
        assert_eq!(lifecycle, expected);
    }
    let (_, _, default_prompt) =
        parse_start_params(&serde_json::json!({ "prompt": "direct" })).unwrap();
    assert_eq!(default_prompt, loopal_runtime::LifecycleMode::Ephemeral);
}

#[test]
fn lifecycle_parse_error_lists_every_supported_wire_value() {
    let error = parse_error(serde_json::json!({ "lifecycle": "unknown" }));
    assert!(error.contains("ephemeral"));
    assert!(error.contains("persistent"));
    assert!(error.contains("workflow_ephemeral"));
}

#[test]
fn accepts_read_only_and_disabled_sandbox_policies() {
    for (wire, expected) in [
        ("read_only", SandboxPolicy::ReadOnly),
        ("disabled", SandboxPolicy::Disabled),
    ] {
        let start = parse(serde_json::json!({ "sandbox_policy": wire })).unwrap();

        assert_eq!(start.sandbox_policy, Some(expected));
    }
}

#[test]
fn no_sandbox_accepts_the_equivalent_disabled_policy() {
    let start = parse(serde_json::json!({
        "no_sandbox": true,
        "sandbox_policy": "disabled",
    }))
    .unwrap();

    assert!(start.no_sandbox);
    assert_eq!(start.sandbox_policy, Some(SandboxPolicy::Disabled));
}

#[test]
fn rejects_unknown_sandbox_policy() {
    let error = parse_error(serde_json::json!({ "sandbox_policy": "unknown" }));

    assert!(error.contains("invalid sandbox policy"));
}

#[test]
fn rejects_non_string_sandbox_policy() {
    let error = parse_error(serde_json::json!({ "sandbox_policy": 42 }));

    assert_eq!(error, "sandbox_policy must be a string");
}

#[test]
fn rejects_sandbox_policy_conflicting_with_no_sandbox() {
    let error = parse_error(serde_json::json!({
        "no_sandbox": true,
        "sandbox_policy": "read_only",
    }));

    assert_eq!(error, "no_sandbox conflicts with sandbox_policy");
}

#[test]
fn accepts_uuid_session_id() {
    let id = uuid::Uuid::new_v4();
    let start = parse(serde_json::json!({ "session_id": id.to_string() })).unwrap();

    assert_eq!(start.session_id, Some(id));
}

#[test]
fn rejects_malformed_session_id() {
    let error = parse_error(serde_json::json!({ "session_id": "not-a-uuid" }));

    assert_eq!(error, "session_id must be a UUID");
}

#[test]
fn rejects_non_string_session_id() {
    let error = parse_error(serde_json::json!({ "session_id": 42 }));

    assert_eq!(error, "session_id must be a string");
}

#[test]
fn workflow_permission_causation_is_strict_and_preserved() {
    let causation = WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_test"),
        node_id: WorkflowNodeId::new("node_test"),
        attempt_id: WorkflowAttemptId::new("watt_test"),
    };
    let capability = WorkflowAttemptCapability::parse("11".repeat(32)).unwrap();
    let start = parse(serde_json::json!({
        "workflow_permission_causation": causation,
        "workflow_attempt_capability": capability.expose(),
        "workflow_completion_result_limit": 128_000,
    }))
    .unwrap();
    assert_eq!(start.workflow_permission_causation, Some(causation));
    assert_eq!(start.workflow_attempt_capability, Some(capability.clone()));
    assert_eq!(start.workflow_completion_result_limit, Some(128_000));

    for invalid in [
        serde_json::json!("not-an-object"),
        serde_json::json!({
            "run_id": "../bad",
            "node_id": "node_test",
            "attempt_id": "watt_test",
        }),
    ] {
        let error = parse_error(serde_json::json!({
            "workflow_permission_causation": invalid,
            "workflow_attempt_capability": capability.expose(),
        }));
        assert_eq!(error, "invalid workflow permission causation");
    }
}

#[test]
fn workflow_authority_fields_must_be_supplied_together() {
    let causation = WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_pairing"),
        node_id: WorkflowNodeId::new("node_pairing"),
        attempt_id: WorkflowAttemptId::new("watt_pairing"),
    };
    let capability = WorkflowAttemptCapability::parse("12".repeat(32)).unwrap();

    for params in [
        serde_json::json!({
            "workflow_permission_causation": causation,
        }),
        serde_json::json!({
            "workflow_attempt_capability": capability.expose(),
        }),
        serde_json::json!({
            "workflow_permission_causation": causation,
            "workflow_attempt_capability": capability.expose(),
        }),
        serde_json::json!({
            "workflow_completion_result_limit": 128_000,
        }),
    ] {
        assert_eq!(
            parse_error(params),
            "workflow authority and completion result limit must be supplied together"
        );
    }
}
