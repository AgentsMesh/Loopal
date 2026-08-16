#[test]
fn child_workflow_events_and_snapshots_are_not_consumed() {
    let child = loopal_tui::view_client::ViewClient::empty("worker");
    let event = AgentEvent::named(
        "worker",
        AgentEventPayload::WorkflowRunChanged(summary("wrun_child", WorkflowRunState::Running, 1)),
    );
    assert!(matches!(
        child.apply_event(&event),
        loopal_view_state::ViewStateApplyOutcome::NoOp
    ));

    let mut state = SessionViewState::empty("worker");
    state
        .workflows
        .active
        .push(summary("wrun_snapshot", WorkflowRunState::Running, 1));
    child.reset_to_snapshot(ViewSnapshot { rev: 3, state });
    assert!(child.workflow_snapshots().is_empty());
    assert!(child.state().state().workflows.is_empty());

    let remote = AgentEvent::named(
        "remote/main",
        AgentEventPayload::WorkflowRunChanged(summary("wrun_remote", WorkflowRunState::Running, 1)),
    );
    let root = loopal_tui::view_client::ViewClient::empty("main");
    assert!(matches!(
        root.apply_event(&remote),
        loopal_view_state::ViewStateApplyOutcome::NoOp
    ));
    assert!(root.workflow_snapshots().is_empty());
}

#[test]
fn renderer_shows_active_and_recent_runs_with_bounded_height() {
    let snapshot = WorkflowRunsSnapshot {
        active: vec![summary("wrun_active", WorkflowRunState::Running, 2)],
        recent: vec![summary("wrun_done", WorkflowRunState::Succeeded, 4)],
    };
    assert_eq!(workflow_ids(&snapshot), vec!["wrun_active", "wrun_done"]);
    assert_eq!(workflows_panel_height(&snapshot), 2);
    let many = WorkflowRunsSnapshot {
        active: (0..10)
            .map(|index| summary(&format!("wrun_{index}"), WorkflowRunState::Running, 1))
            .collect(),
        recent: Vec::new(),
    };
    assert_eq!(workflows_panel_height(&many), MAX_WORKFLOW_VISIBLE as u16);

    let mut terminal = Terminal::new(TestBackend::new(80, 2)).unwrap();
    terminal
        .draw(|frame| {
            render_workflows_panel(
                frame,
                &snapshot,
                Some("wrun_active"),
                0,
                Rect::new(0, 0, 80, 2),
            );
        })
        .unwrap();
    let text = terminal.backend().to_string();
    assert!(text.contains("goal for wrun_active"), "{text}");
    assert!(text.contains("running"), "{text}");
    assert!(text.contains("goal for wrun_done"), "{text}");
    assert!(text.contains("done"), "{text}");
}
