use loopal_protocol::{
    MAX_WORKFLOW_OUTPUT_BYTES, WorkflowAttemptCapability, WorkflowAttemptId, WorkflowNodeId,
    WorkflowPermissionCausation, WorkflowRunId,
};

use super::parse_start_params;

fn workflow_params(limit: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "workflow_permission_causation": WorkflowPermissionCausation {
            run_id: WorkflowRunId::new("wrun_limit"),
            node_id: WorkflowNodeId::new("wnode_limit"),
            attempt_id: WorkflowAttemptId::new("watt_limit"),
        },
        "workflow_attempt_capability": WorkflowAttemptCapability::parse("31".repeat(32))
            .unwrap()
            .expose(),
        "workflow_completion_result_limit": limit,
    })
}

#[test]
fn workflow_completion_result_limit_is_protocol_bounded() {
    for invalid in [
        serde_json::json!(0),
        serde_json::json!(u64::from(MAX_WORKFLOW_OUTPUT_BYTES) + 1),
        serde_json::json!("large"),
    ] {
        let error = match parse_start_params(&workflow_params(invalid)) {
            Err(error) => error,
            Ok(_) => panic!("invalid workflow completion limit was accepted"),
        };
        assert_eq!(
            error.to_string(),
            "invalid workflow completion result limit"
        );
    }

    let (start, _, _) = parse_start_params(&workflow_params(serde_json::json!(
        MAX_WORKFLOW_OUTPUT_BYTES
    )))
    .unwrap();
    assert_eq!(
        start.workflow_completion_result_limit,
        Some(MAX_WORKFLOW_OUTPUT_BYTES)
    );
}
