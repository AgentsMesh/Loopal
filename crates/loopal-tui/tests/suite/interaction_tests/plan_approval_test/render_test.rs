use super::*;

#[test]
fn pending_plan_renders_path_content_and_actions() {
    let mut app = make_app();
    app.show_topology = false;
    request_plan(&mut app, "# Plan\n1. Inspect\n2. Implement");
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|frame| loopal_tui::render::draw(frame, &mut app))
        .unwrap();
    let text = buffer_text(&terminal);
    assert!(text.contains("Review plan"), "buffer was:\n{text}");
    assert!(text.contains("/tmp/plans/feature.md"));
    assert!(text.contains("1. Inspect"));
    assert!(text.contains("Approve [y]"));
    assert!(text.contains("Reject [n]"));
}

#[test]
fn long_plan_can_scroll_through_all_content() {
    let plan = PendingPlanApproval {
        id: "plan-1".into(),
        plan_content: (1..=12)
            .map(|line| format!("step-{line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        plan_path: "/tmp/plan.md".into(),
    };
    assert_eq!(plan_approval_inline::height(&plan, 80), 10);
    let mut terminal = Terminal::new(TestBackend::new(80, 10)).unwrap();
    terminal
        .draw(|frame| plan_approval_inline::render(frame, &plan, 5, Rect::new(0, 0, 80, 10), None))
        .unwrap();
    let text = buffer_text(&terminal);
    assert!(text.contains("step-6"));
    assert!(text.contains("step-12"));
    assert!(text.contains("Lines 6-12/12"));
}

#[test]
fn small_plan_viewports_can_scroll_to_last_line() {
    let plan = PendingPlanApproval {
        id: "plan-1".into(),
        plan_content: (1..=12)
            .map(|line| format!("step-{line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        plan_path: "/tmp/plan.md".into(),
    };

    for height in 4..=9 {
        let viewport = plan_approval_inline::content_viewport_rows(height);
        let scroll = plan_approval_inline::max_scroll(&plan, viewport);
        let mut terminal = Terminal::new(TestBackend::new(80, height)).unwrap();
        terminal
            .draw(|frame| {
                plan_approval_inline::render(
                    frame,
                    &plan,
                    scroll,
                    Rect::new(0, 0, 80, height),
                    None,
                )
            })
            .unwrap();
        let text = buffer_text(&terminal);
        assert!(
            text.contains("step-12"),
            "height {height} did not reach the last line:\n{text}"
        );
        let first_visible = 13 - viewport;
        assert!(
            text.contains(&format!("Lines {first_visible}-12/12")),
            "height {height} reported the wrong viewport:\n{text}"
        );
    }
}

#[tokio::test]
async fn plan_scroll_uses_last_rendered_viewport() {
    let mut app = make_app();
    let content = (1..=12)
        .map(|line| format!("step-{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    request_plan(&mut app, &content);
    app.plan_approval_viewport_rows = 2;

    key_dispatch_for_test::dispatch(&mut app, InputAction::PlanScroll(99)).await;

    assert_eq!(app.plan_approval_scroll, 10);
}

#[tokio::test]
async fn app_small_terminal_scroll_reaches_last_plan_line() {
    let mut app = make_app();
    app.show_topology = false;
    let content = (1..=12)
        .map(|line| format!("step-{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    request_plan(&mut app, &content);
    let mut terminal = Terminal::new(TestBackend::new(80, 9)).unwrap();
    terminal
        .draw(|frame| loopal_tui::render::draw(frame, &mut app))
        .unwrap();
    assert!(
        (1..plan_approval_inline::content_viewport_rows(10))
            .contains(&app.plan_approval_viewport_rows),
        "expected a constrained plan viewport, got {} rows",
        app.plan_approval_viewport_rows
    );

    key_dispatch_for_test::dispatch(&mut app, InputAction::PlanScroll(99)).await;
    terminal
        .draw(|frame| loopal_tui::render::draw(frame, &mut app))
        .unwrap();

    let text = buffer_text(&terminal);
    assert!(text.contains("step-12"), "buffer was:\n{text}");
}
