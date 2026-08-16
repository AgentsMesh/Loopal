use std::sync::Arc;

use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, ProjectedMessage, SubAgentSpawn,
    UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::views::plan_approval_inline;
use tokio::sync::{mpsc, watch};

fn app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(8);
    let (permission_tx, _) = mpsc::channel::<bool>(8);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(8);
    let (interrupt_tx, _) = watch::channel(0_u64);
    App::new(
        SessionController::new(
            control_tx,
            permission_tx,
            question_tx,
            Default::default(),
            Arc::new(interrupt_tx),
        ),
        std::env::temp_dir(),
    )
}

fn plan(id: &str) -> AgentEventPayload {
    AgentEventPayload::PlanApprovalRequest {
        id: id.into(),
        plan_content: "one\ntwo".into(),
        plan_path: "/tmp/plan.md".into(),
    }
}

#[test]
fn plan_scroll_reset_is_scoped_to_the_active_agent_and_matching_id() {
    let mut app = app();
    app.dispatch_event(AgentEvent::named(
        "worker",
        AgentEventPayload::SubAgentSpawned(SubAgentSpawn {
            name: "worker".into(),
            agent_id: "agent-worker".into(),
            parent: Some("main".into()),
            model: None,
            session_id: None,
        }),
    ));
    assert!(app.session.enter_agent_view("worker"));
    app.plan_approval_scroll = 8;
    app.plan_approval_viewport_rows = 2;

    app.dispatch_event(AgentEvent::root(plan("root-plan")));
    assert_eq!(app.plan_approval_scroll, 8);
    assert_eq!(app.plan_approval_viewport_rows, 2);

    app.dispatch_event(AgentEvent::named("worker", plan("worker-plan")));
    assert_eq!(app.plan_approval_scroll, 0);
    assert_eq!(
        app.plan_approval_viewport_rows,
        plan_approval_inline::content_viewport_rows(10)
    );

    app.plan_approval_scroll = 6;
    app.plan_approval_viewport_rows = 1;
    app.dispatch_event(AgentEvent::named(
        "worker",
        AgentEventPayload::PlanApprovalResolved { id: "stale".into() },
    ));
    assert_eq!(app.plan_approval_scroll, 6);
    app.dispatch_event(AgentEvent::named(
        "worker",
        AgentEventPayload::PlanApprovalResolved {
            id: "worker-plan".into(),
        },
    ));
    assert_eq!(app.plan_approval_scroll, 0);
    assert_eq!(
        app.plan_approval_viewport_rows,
        plan_approval_inline::content_viewport_rows(10)
    );
}

#[test]
fn history_load_is_a_noop_when_the_root_view_has_been_removed() {
    let mut app = app();
    app.view_clients.shift_remove("main");
    app.load_display_history(vec![ProjectedMessage {
        role: "assistant".into(),
        content: "detached history".into(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
    }]);
    assert!(app.view_clients.is_empty());

    assert!(!app.dispatch_event(AgentEvent::root(AgentEventPayload::Started)));
}
