//! Tests for section header rendering + `panel_zone_height`.

use loopal_protocol::{
    BgTaskStatus, ControlCommand, TaskSnapshot, TaskSnapshotStatus, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode, PanelKind};
use loopal_tui::render_panel::{panel_zone_height, render_panel_zone};
use loopal_tui::views::panel_header::render_section_header;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::prelude::*;

use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn task(id: &str, status: TaskSnapshotStatus) -> TaskSnapshot {
    TaskSnapshot {
        id: id.into(),
        subject: format!("Task {id}"),
        active_form: None,
        status,
        blocked_by: Vec::new(),
    }
}

fn bg(id: &str) -> loopal_protocol::BgTaskSnapshot {
    loopal_protocol::BgTaskSnapshot {
        id: id.into(),
        description: format!("desc {id}"),
        status: BgTaskStatus::Running,
        exit_code: None,
    }
}

fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

// ── render_section_header ────────────────────────────────────────────

#[test]
fn section_header_includes_title_and_count() {
    let backend = TestBackend::new(40, 1);
    let mut t = Terminal::new(backend).unwrap();
    t.draw(|f| render_section_header(f, "Tasks", 3, false, Rect::new(0, 0, 40, 1)))
        .unwrap();
    let text = buffer_text(&t);
    assert!(text.contains("Tasks"), "{text}");
    assert!(text.contains("(3)"), "{text}");
}

#[test]
fn section_header_omits_count_when_zero() {
    let backend = TestBackend::new(40, 1);
    let mut t = Terminal::new(backend).unwrap();
    t.draw(|f| render_section_header(f, "Agents", 0, false, Rect::new(0, 0, 40, 1)))
        .unwrap();
    assert!(!buffer_text(&t).contains("(0)"));
}

#[test]
fn section_header_focused_uses_cyan_title() {
    let backend = TestBackend::new(40, 1);
    let mut t = Terminal::new(backend).unwrap();
    t.draw(|f| render_section_header(f, "Tasks", 1, true, Rect::new(0, 0, 40, 1)))
        .unwrap();
    let buf = t.backend().buffer();
    // Find cell containing 'T' from "Tasks" — it should be Cyan.
    let found = (0..buf.area.width)
        .find(|&x| buf[(x, 0)].symbol() == "T")
        .map(|x| buf[(x, 0)].fg);
    assert_eq!(found, Some(Color::Cyan));
}

#[test]
fn section_header_unfocused_not_cyan_title() {
    let backend = TestBackend::new(40, 1);
    let mut t = Terminal::new(backend).unwrap();
    t.draw(|f| render_section_header(f, "Tasks", 1, false, Rect::new(0, 0, 40, 1)))
        .unwrap();
    let buf = t.backend().buffer();
    let found = (0..buf.area.width)
        .find(|&x| buf[(x, 0)].symbol() == "T")
        .map(|x| buf[(x, 0)].fg);
    assert_ne!(found, Some(Color::Cyan));
}

#[test]
fn section_header_empty_area_is_noop() {
    let backend = TestBackend::new(40, 1);
    let mut t = Terminal::new(backend).unwrap();
    t.draw(|f| render_section_header(f, "Tasks", 1, true, Rect::new(0, 0, 0, 1)))
        .unwrap();
    // No panic; buffer remains the default spaces.
    assert!(!buffer_text(&t).contains("Tasks"));
}

// ── panel_zone_height ────────────────────────────────────────────────

#[test]
fn zone_height_zero_when_all_empty() {
    let app = make_app();
    let state = app.session.lock();
    assert_eq!(panel_zone_height(&app, &state), 0);
}

#[test]
fn zone_height_single_panel_no_header_added() {
    let app = make_app();
    app.view_clients["main"].inject_tasks_for_test(vec![task("1", TaskSnapshotStatus::InProgress)]);
    let state = app.session.lock();
    assert_eq!(panel_zone_height(&app, &state), 1);
}

#[test]
fn zone_height_two_panels_adds_two_header_rows() {
    let app = make_app();
    app.view_clients["main"].inject_tasks_for_test(vec![task("1", TaskSnapshotStatus::InProgress)]);
    app.view_clients["main"].inject_bg_for_test(vec![bg("bg_1")]);
    let state = app.session.lock();
    // 1 tasks + 1 bg + 2 headers = 4
    assert_eq!(panel_zone_height(&app, &state), 4);
}

// ── end-to-end: render_panel_zone with headers ───────────────────────

#[test]
fn render_two_panels_produces_two_header_lines() {
    let mut app = make_app();
    app.view_clients["main"].inject_tasks_for_test(vec![task("1", TaskSnapshotStatus::InProgress)]);
    app.view_clients["main"].inject_bg_for_test(vec![bg("bg_1")]);
    app.focus_mode = FocusMode::Panel(PanelKind::Tasks);

    let backend = TestBackend::new(50, 4);
    let mut t = Terminal::new(backend).unwrap();
    {
        let state = app.session.lock();
        t.draw(|f| {
            render_panel_zone(
                f,
                &app,
                &state,
                std::time::Duration::ZERO,
                Rect::new(0, 0, 50, 4),
            )
        })
        .unwrap();
    }
    let text = buffer_text(&t);
    assert!(text.contains("Tasks"), "tasks header missing: {text}");
    assert!(text.contains("Background"), "bg header missing: {text}");
    assert!(text.contains("━"), "decor lines missing: {text}");
}
