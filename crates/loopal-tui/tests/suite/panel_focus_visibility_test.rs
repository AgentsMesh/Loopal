//! Verifies that Tab-switching between sub-panels hides the ` ▸ `
//! indicator on non-active panels, preventing the "arrows on multiple
//! panels simultaneously" visual bug.
//!
//! Strategy: populate both Tasks and BgTasks, set both `section.focused`,
//! render the panel zone, and count the ` ▸ ` occurrences — it must
//! equal exactly one (the active panel) or zero (in Input mode).

use loopal_protocol::{BgTaskDetail, BgTaskStatus, ControlCommand, UserQuestionResponse};
use loopal_protocol::{TaskSnapshot, TaskSnapshotStatus};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode, PanelKind};
use loopal_tui::render_panel::render_panel_zone;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;

use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "m".into(),
        "act".into(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn setup_two_panels() -> App {
    let mut app = make_app();
    app.task_snapshots = vec![TaskSnapshot {
        id: "1".into(),
        subject: "Task 1".into(),
        active_form: None,
        status: TaskSnapshotStatus::InProgress,
        blocked_by: Vec::new(),
    }];
    app.session.lock().bg_tasks.insert(
        "bg_1".into(),
        BgTaskDetail {
            id: "bg_1".into(),
            description: "bg".into(),
            status: BgTaskStatus::Running,
            exit_code: None,
            output: String::new(),
        },
    );
    app.bg_snapshots = app
        .session
        .lock()
        .bg_tasks
        .values()
        .map(|t| t.to_snapshot())
        .collect();
    // Both panels have a remembered selection in state.
    app.section_mut(PanelKind::Tasks).focused = Some("1".into());
    app.section_mut(PanelKind::BgTasks).focused = Some("bg_1".into());
    app
}

fn render_and_dump(app: &App, width: u16, height: u16) -> String {
    let state = app.session.lock();
    let backend = TestBackend::new(width, height);
    let mut t = Terminal::new(backend).unwrap();
    t.draw(|f| {
        render_panel_zone(
            f,
            app,
            &state,
            std::time::Duration::ZERO,
            Rect::new(0, 0, width, height),
        )
    })
    .unwrap();
    let buf = t.backend().buffer();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn count_arrows(text: &str) -> usize {
    text.matches('▸').count()
}

#[test]
fn only_active_panel_shows_arrow_when_tasks_focused() {
    let mut app = setup_two_panels();
    app.focus_mode = FocusMode::Panel(PanelKind::Tasks);
    let text = render_and_dump(&app, 80, 4);
    assert_eq!(
        count_arrows(&text),
        1,
        "exactly one ▸ expected, got:\n{text}"
    );
}

#[test]
fn only_active_panel_shows_arrow_when_bg_focused() {
    let mut app = setup_two_panels();
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);
    let text = render_and_dump(&app, 80, 4);
    assert_eq!(
        count_arrows(&text),
        1,
        "exactly one ▸ expected, got:\n{text}"
    );
}

#[test]
fn no_arrows_in_input_mode() {
    let app = setup_two_panels();
    // focus_mode defaults to Input; both sections still have `focused` set.
    let text = render_and_dump(&app, 80, 4);
    assert_eq!(count_arrows(&text), 0, "no ▸ expected, got:\n{text}");
}

#[test]
fn state_preserved_across_render() {
    // Rendering must not mutate `section.focused` — selection is restored
    // when the user Tabs back to that panel.
    let mut app = setup_two_panels();
    app.focus_mode = FocusMode::Panel(PanelKind::Tasks);
    let _ = render_and_dump(&app, 80, 4);
    assert_eq!(
        app.section(PanelKind::BgTasks).focused.as_deref(),
        Some("bg_1")
    );
    assert_eq!(app.section(PanelKind::Tasks).focused.as_deref(), Some("1"));
}

#[test]
fn arrow_follows_tab_switch() {
    let mut app = setup_two_panels();
    app.focus_mode = FocusMode::Panel(PanelKind::Tasks);
    let before = render_and_dump(&app, 80, 4);
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);
    let after = render_and_dump(&app, 80, 4);
    // Arrow on each render is 1, but located in different rows.
    assert_eq!(count_arrows(&before), 1);
    assert_eq!(count_arrows(&after), 1);
    // Arrow row should differ — Tasks panel is above BgTasks panel.
    let row_of = |t: &str| {
        t.lines()
            .enumerate()
            .find_map(|(i, l)| if l.contains('▸') { Some(i) } else { None })
    };
    assert_ne!(row_of(&before), row_of(&after));
}

#[test]
fn single_panel_has_no_header() {
    // Only Tasks has content → no "━━ Tasks ━━" header should appear.
    let mut app = make_app();
    app.task_snapshots = vec![TaskSnapshot {
        id: "1".into(),
        subject: "Only task".into(),
        active_form: None,
        status: TaskSnapshotStatus::InProgress,
        blocked_by: Vec::new(),
    }];
    let text = render_and_dump(&app, 40, 2);
    assert!(
        !text.contains("━"),
        "single panel should not show header decor: {text}"
    );
    assert!(
        !text.contains("Tasks"),
        "single panel should not show title label: {text}"
    );
}
