use super::*;

#[test]
fn plan_modal_routes_approve_reject_and_scroll_keys() {
    let mut app = make_app();
    request_plan(&mut app, "step");
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Char('y'))),
        InputAction::PlanApprove
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Char('n'))),
        InputAction::PlanReject
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Esc)),
        InputAction::PlanReject
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Down)),
        InputAction::PlanScroll(1)
    ));
}

#[tokio::test]
async fn approve_keeps_pending_until_resolved_event() {
    let mut app = make_app();
    let content = (1..=12)
        .map(|line| format!("step-{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    request_plan(&mut app, &content);
    key_dispatch_for_test::dispatch(&mut app, InputAction::PlanScroll(99)).await;
    assert_eq!(app.plan_approval_scroll, 5);
    app.plan_approval_viewport_rows = 2;
    key_dispatch_for_test::dispatch(&mut app, InputAction::PlanApprove).await;
    assert!(
        app.with_active_conversation(|conv| conv.pending_plan_approval.is_some()),
        "pending must stay visible until Hub confirms resolution"
    );

    app.dispatch_event(AgentEvent::root(AgentEventPayload::PlanApprovalResolved {
        id: "stale-plan".into(),
    }));
    assert_eq!(app.plan_approval_scroll, 5);
    assert_eq!(app.plan_approval_viewport_rows, 2);
    assert!(app.with_active_conversation(|conv| conv.pending_plan_approval.is_some()));

    app.dispatch_event(AgentEvent::root(AgentEventPayload::PlanApprovalResolved {
        id: "plan-1".into(),
    }));
    assert_eq!(app.plan_approval_scroll, 0);
    assert_eq!(
        app.plan_approval_viewport_rows,
        plan_approval_inline::content_viewport_rows(10)
    );
    assert!(app.with_active_conversation(|conv| conv.pending_plan_approval.is_none()));
}

#[tokio::test]
async fn reject_clears_only_after_resolved_event() {
    let mut app = make_app();
    request_plan(&mut app, "# Plan");
    key_dispatch_for_test::dispatch(&mut app, InputAction::PlanReject).await;
    assert!(app.with_active_conversation(|conv| conv.pending_plan_approval.is_some()));
    app.dispatch_event(AgentEvent::root(AgentEventPayload::PlanApprovalResolved {
        id: "plan-1".into(),
    }));
    assert!(app.with_active_conversation(|conv| conv.pending_plan_approval.is_none()));
}
