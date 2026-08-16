use loopal_agent_client::{StartAgentParams, encode};
use loopal_protocol::{
    WorkflowAttemptId, WorkflowNodeId, WorkflowPermissionCausation, WorkflowRunId,
};

#[test]
fn workflow_permission_causation_is_preserved_on_start_wire() {
    let causation = WorkflowPermissionCausation {
        run_id: WorkflowRunId::new("wrun_test"),
        node_id: WorkflowNodeId::new("node_test"),
        attempt_id: WorkflowAttemptId::new("watt_test"),
    };
    let wire = encode(&StartAgentParams {
        workflow_permission_causation: Some(causation.clone()),
        workflow_completion_result_limit: Some(256_000),
        ..Default::default()
    });

    assert_eq!(
        serde_json::from_value::<WorkflowPermissionCausation>(
            wire["workflow_permission_causation"].clone()
        )
        .unwrap(),
        causation
    );
    assert_eq!(wire["workflow_completion_result_limit"], 256_000);
}

#[test]
fn ordinary_start_has_no_workflow_permission_authority() {
    let wire = encode(&StartAgentParams::default());
    assert!(wire["workflow_permission_causation"].is_null());
    assert!(wire["workflow_completion_result_limit"].is_null());
}

#[test]
fn fork_context_is_only_added_when_present() {
    let context = serde_json::json!({"messages": [{"role": "user", "content": "review"}]});
    let wire = encode(&StartAgentParams {
        fork_context: Some(context.clone()),
        ..Default::default()
    });
    assert_eq!(wire["fork_context"], context);
    assert!(
        encode(&StartAgentParams::default())
            .get("fork_context")
            .is_none()
    );
}
