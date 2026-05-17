use loopal_protocol::{AgentEventPayload, BgTaskDetail, BgTaskStatus};
use loopal_view_state::{BgTaskView, ViewStateReducer};

fn spawn(id: &str, description: &str) -> AgentEventPayload {
    AgentEventPayload::BgTaskSpawned {
        id: id.into(),
        description: description.into(),

        created_at_unix_ms: 0,
    }
}

#[test]
fn bg_task_spawned_inserts_running_view() {
    let mut r = ViewStateReducer::new("root");
    r.apply(spawn("bg_1", "long-running build"));
    let view = r.state().bg_tasks.get("bg_1").expect("bg_1 inserted");
    assert_eq!(view.description, "long-running build");
    assert!(matches!(view.status, BgTaskStatus::Running));
    assert!(view.output.is_empty());
}

#[test]
fn bg_task_output_appends_incrementally() {
    let mut r = ViewStateReducer::new("root");
    r.apply(spawn("bg_1", "tail"));
    r.apply(AgentEventPayload::BgTaskOutput {
        id: "bg_1".into(),
        output_delta: "line1\n".into(),
    });
    r.apply(AgentEventPayload::BgTaskOutput {
        id: "bg_1".into(),
        output_delta: "line2\n".into(),
    });
    assert_eq!(r.state().bg_tasks["bg_1"].output, "line1\nline2\n");
}

#[test]
fn bg_task_output_for_unknown_id_returns_none() {
    let mut r = ViewStateReducer::new("root");
    let result = r.apply(AgentEventPayload::BgTaskOutput {
        id: "missing".into(),
        output_delta: "x".into(),
    });
    assert!(result.is_none());
    assert_eq!(r.rev(), 0);
}

#[test]
fn bg_task_completed_replaces_output_and_sets_status() {
    let mut r = ViewStateReducer::new("root");
    r.apply(spawn("bg_1", "build"));
    r.apply(AgentEventPayload::BgTaskOutput {
        id: "bg_1".into(),
        output_delta: "partial".into(),
    });
    r.apply(AgentEventPayload::BgTaskCompleted {
        id: "bg_1".into(),
        status: BgTaskStatus::Completed,
        exit_code: Some(0),
        output: "final transcript".into(),
    });
    let view = &r.state().bg_tasks["bg_1"];
    assert!(matches!(view.status, BgTaskStatus::Completed));
    assert_eq!(view.exit_code, Some(0));
    assert_eq!(view.output, "final transcript");
}

#[test]
fn bg_task_completed_for_unknown_id_creates_placeholder() {
    let mut r = ViewStateReducer::new("root");
    let result = r.apply(AgentEventPayload::BgTaskCompleted {
        id: "ghost".into(),
        status: BgTaskStatus::Failed,
        exit_code: Some(1),
        output: "".into(),
    });
    assert!(result.is_some());
    let view = r
        .state()
        .bg_tasks
        .get("ghost")
        .expect("placeholder created");
    assert!(matches!(view.status, BgTaskStatus::Failed));
    assert_eq!(view.exit_code, Some(1));
}

#[test]
fn bg_view_from_detail_carries_output() {
    let detail = BgTaskDetail {
        id: "bg_x".into(),
        description: "captured run".into(),

        status: BgTaskStatus::Completed,
        exit_code: Some(0),
        output: "transcript text".into(),
        created_at_unix_ms: 0,
    };
    let view = BgTaskView::from_detail(detail);
    assert_eq!(view.id, "bg_x");
    assert_eq!(view.output, "transcript text");
    assert!(matches!(view.status, BgTaskStatus::Completed));
}
