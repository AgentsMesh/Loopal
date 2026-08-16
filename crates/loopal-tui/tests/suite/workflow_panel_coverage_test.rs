use loopal_protocol::{
    WorkflowNodeId, WorkflowRunId, WorkflowRunState, WorkflowRunSummary, WorkflowRunsSnapshot,
    WorkflowStateCounts,
};
use loopal_tui::views::workflows_panel::{
    MAX_WORKFLOW_VISIBLE, render_workflows_panel, workflow_ids, workflows_panel_height,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::{Color, Rect};

fn summary(id: &str, state: WorkflowRunState) -> WorkflowRunSummary {
    WorkflowRunSummary {
        id: WorkflowRunId::new(id),
        run_goal: format!("goal-{id}"),
        state,
        revision: 1,
        output_node: WorkflowNodeId::new("done"),
        counts: WorkflowStateCounts {
            pending: 0,
            ready: 0,
            active: 0,
            succeeded: 0,
            failed: 0,
            cancelled: 0,
            skipped: 0,
        },
        created_at_unix_ms: 1,
        updated_at_unix_ms: 1,
    }
}

fn render(
    snapshot: &WorkflowRunsSnapshot,
    focused: Option<&str>,
    offset: usize,
) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(72, MAX_WORKFLOW_VISIBLE as u16)).unwrap();
    terminal
        .draw(|frame| {
            render_workflows_panel(
                frame,
                snapshot,
                focused,
                offset,
                Rect::new(0, 0, 72, MAX_WORKFLOW_VISIBLE as u16),
            );
        })
        .unwrap();
    terminal
}

#[test]
fn renderer_covers_all_states_focus_styles_and_clamped_scrolling() {
    let states = [
        ("planned", WorkflowRunState::Planned),
        ("validated", WorkflowRunState::Validated),
        ("running", WorkflowRunState::Running),
        ("cancelling", WorkflowRunState::Cancelling),
        ("succeeded", WorkflowRunState::Succeeded),
        ("failed", WorkflowRunState::Failed),
        ("cancelled", WorkflowRunState::Cancelled),
    ];
    let snapshot = WorkflowRunsSnapshot {
        active: states
            .into_iter()
            .map(|(id, state)| summary(id, state))
            .collect(),
        recent: Vec::new(),
    };

    let first = render(&snapshot, Some("planned"), 0);
    let first_text = first.backend().to_string();
    assert!(first_text.contains("planned"), "{first_text}");
    assert!(first_text.contains("ready"), "{first_text}");
    assert!(first_text.contains("running"), "{first_text}");
    assert!(first_text.contains("stopping"), "{first_text}");
    assert!(first_text.contains("done"), "{first_text}");
    assert_eq!(first.backend().buffer()[(1, 0)].symbol(), ">");
    assert_eq!(first.backend().buffer()[(1, 0)].fg, Color::Cyan);

    let last = render(&snapshot, None, usize::MAX);
    let last_text = last.backend().to_string();
    assert!(last_text.contains("failed"), "{last_text}");
    assert!(last_text.contains("cancelled"), "{last_text}");
    assert!(!last_text.contains("goal-planned"), "{last_text}");
}

#[test]
fn empty_and_zero_height_panels_are_bounded_noops() {
    let snapshot = WorkflowRunsSnapshot::default();
    assert!(workflow_ids(&snapshot).is_empty());
    assert_eq!(workflows_panel_height(&snapshot), 0);

    let mut terminal = Terminal::new(TestBackend::new(8, 1)).unwrap();
    terminal
        .draw(|frame| {
            render_workflows_panel(frame, &snapshot, None, 99, Rect::new(0, 0, 8, 1));
            render_workflows_panel(frame, &snapshot, None, 0, Rect::new(0, 0, 8, 0));
        })
        .unwrap();
    for x in 0..8 {
        assert_eq!(terminal.backend().buffer()[(x, 0)].symbol(), " ");
    }
}
