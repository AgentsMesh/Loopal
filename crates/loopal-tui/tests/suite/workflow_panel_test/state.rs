#[test]
fn workflow_panel_reads_snapshot_state_and_is_registered() {
    let mut app = make_app();
    let active = summary("wrun_active", WorkflowRunState::Running, 2);
    let recent = summary("wrun_done", WorkflowRunState::Succeeded, 4);
    let mut state = SessionViewState::empty("main");
    state.workflows = WorkflowRunsSnapshot {
        active: vec![active.clone()],
        recent: vec![recent.clone()],
    };
    app.view_clients["main"].reset_to_snapshot(ViewSnapshot { rev: 9, state });

    assert_eq!(
        app.view_clients["main"].workflow_snapshots(),
        WorkflowRunsSnapshot {
            active: vec![active],
            recent: vec![recent],
        }
    );
    let session = app.session.lock();
    let provider = app
        .panel_registry
        .by_kind(PanelKind::Workflows)
        .expect("workflow provider registered");
    assert_eq!(provider.kind(), PanelKind::Workflows);
    assert_eq!(provider.title(), "Workflows");
    assert_eq!(provider.max_visible(), MAX_WORKFLOW_VISIBLE);
    assert_eq!(
        provider.item_ids(&app, &session),
        vec!["wrun_active", "wrun_done"]
    );
    assert_eq!(provider.count(&app, &session), 2);
    assert_eq!(provider.height(&app, &session), 2);
    let mut terminal = Terminal::new(TestBackend::new(80, 2)).unwrap();
    terminal
        .draw(|frame| {
            provider.render(
                frame,
                &app,
                &session,
                Some("wrun_active"),
                std::time::Duration::ZERO,
                Rect::new(0, 0, 80, 2),
            );
        })
        .unwrap();
    assert!(
        terminal
            .backend()
            .to_string()
            .contains("goal for wrun_active")
    );
    drop(session);

    app.section_mut(PanelKind::Workflows).focused = Some("wrun_active".into());
}

#[test]
fn live_event_updates_visible_run_and_gap_requires_snapshot_replacement() {
    let mut app = make_app();
    let first = summary("wrun_live", WorkflowRunState::Running, 2);
    assert!(
        !app.dispatch_event(AgentEvent::root(AgentEventPayload::WorkflowRunChanged(
            first
        ),))
    );
    assert_eq!(
        app.view_clients["main"].workflow_snapshots().active[0].revision,
        2
    );

    let gap = summary("wrun_live", WorkflowRunState::Succeeded, 4);
    assert!(app.dispatch_event(AgentEvent::root(AgentEventPayload::WorkflowRunChanged(gap),)));
    let projected = app.view_clients["main"].workflow_snapshots();
    assert_eq!(projected.active[0].revision, 2);
    assert!(projected.recent.is_empty());

    let replacement = summary("wrun_live", WorkflowRunState::Succeeded, 4);
    let mut state = SessionViewState::empty("main");
    state.workflows.recent.push(replacement.clone());
    app.view_clients["main"].reset_to_snapshot(ViewSnapshot { rev: 12, state });
    assert_eq!(
        app.view_clients["main"].workflow_snapshots().recent,
        vec![replacement]
    );
}
