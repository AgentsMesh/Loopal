use loopal_protocol::{
    PermissionIntentRequest, WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation,
    WorkflowRunId,
};

use super::PermissionRequest;

fn causation(run: &str, node: &str, attempt: &str) -> WorkflowPermissionCausation {
    WorkflowPermissionCausation {
        run_id: WorkflowRunId::new(run),
        node_id: WorkflowNodeId::new(node),
        attempt_id: WorkflowAttemptId::new(attempt),
    }
}

fn request(workflow: Option<WorkflowPermissionCausation>) -> PermissionRequest {
    let request = PermissionIntentRequest::create(
        "call",
        "Write",
        serde_json::json!({}),
        serde_json::json!({}),
        serde_json::json!({"type": "object"}),
        workflow,
    )
    .unwrap();
    PermissionRequest::parse(serde_json::to_value(request).unwrap()).unwrap()
}

#[test]
fn workflow_authority_requires_exact_registered_causation() {
    let expected = causation("wrun_expected", "wnode_expected", "watt_expected");
    let permission = request(Some(expected.clone()));

    assert!(permission.matches_workflow_authority(Some(&expected)));
    assert!(!permission.matches_workflow_authority(None));
    for other in [
        causation("wrun_other", "wnode_expected", "watt_expected"),
        causation("wrun_expected", "wnode_other", "watt_expected"),
        causation("wrun_expected", "wnode_expected", "watt_other"),
    ] {
        assert!(!permission.matches_workflow_authority(Some(&other)));
    }
}

#[test]
fn direct_request_requires_absent_workflow_authority() {
    let permission = request(None);

    assert!(permission.matches_workflow_authority(None));
    assert!(!permission.matches_workflow_authority(Some(&causation(
        "wrun_test",
        "wnode_test",
        "watt_expected",
    ))));
}

#[test]
fn legacy_request_never_matches_workflow_authority() {
    let permission = PermissionRequest::parse(serde_json::json!({
        "tool_call_id": "legacy",
        "tool_name": "Write",
        "tool_input": {},
    }))
    .unwrap();

    assert!(!permission.matches_workflow_authority(None));
    assert!(!permission.matches_workflow_authority(Some(&causation(
        "wrun_test",
        "wnode_test",
        "watt_expected",
    ))));
}
