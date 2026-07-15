use loopal_protocol::AgentEventPayload;
use loopal_view_state::ViewStateReducer;

#[test]
fn plan_approval_survives_snapshot_and_resolves_by_id() {
    let mut reducer = ViewStateReducer::new("main");
    reducer.apply(AgentEventPayload::PlanApprovalRequest {
        id: "plan-1".into(),
        plan_content: "# Plan\nStep 1".into(),
        plan_path: "/tmp/plan.md".into(),
    });
    let pending = reducer
        .state()
        .agent
        .conversation
        .pending_plan_approval
        .as_ref()
        .unwrap();
    assert_eq!(pending.id, "plan-1");
    assert_eq!(pending.plan_content, "# Plan\nStep 1");
    let json = serde_json::to_value(reducer.state()).unwrap();
    assert_eq!(
        json["agent"]["conversation"]["pending_plan_approval"]["plan_path"],
        "/tmp/plan.md"
    );
    reducer.apply(AgentEventPayload::PlanApprovalResolved { id: "other".into() });
    assert!(
        reducer
            .state()
            .agent
            .conversation
            .pending_plan_approval
            .is_some()
    );
    reducer.apply(AgentEventPayload::PlanApprovalResolved {
        id: "plan-1".into(),
    });
    assert!(
        reducer
            .state()
            .agent
            .conversation
            .pending_plan_approval
            .is_none()
    );
}
