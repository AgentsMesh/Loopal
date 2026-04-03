/// Tests for bg task focus cycling and bg_tasks_panel utility functions.
use loopal_protocol::{BgTaskSnapshot, BgTaskStatus, ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode, PanelKind};
use loopal_tui::dispatch_ops::cycle_panel_focus;
use loopal_tui::views::bg_tasks_panel;

use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "test-model".into(),
        "act".into(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn add_bg_task(app: &App, id: &str, desc: &str) {
    app.bg_store
        .register_proxy(id.to_string(), desc.to_string());
}

fn sync_bg(app: &mut App) {
    app.bg_snapshots = app.bg_store.snapshot_running();
}

fn snap(id: &str, desc: &str) -> BgTaskSnapshot {
    BgTaskSnapshot {
        id: id.into(),
        description: desc.into(),
        status: BgTaskStatus::Running,
    }
}

// === bg_tasks_panel utility functions (pure, no store) ===

#[test]
fn bg_panel_height_zero_when_no_tasks() {
    assert_eq!(bg_tasks_panel::bg_panel_height(&[]), 0);
}

#[test]
fn bg_panel_height_counts_running_tasks() {
    let snaps = vec![snap("bg_1", "one"), snap("bg_2", "two")];
    assert_eq!(bg_tasks_panel::bg_panel_height(&snaps), 2);
}

#[test]
fn bg_panel_height_caps_at_max() {
    let snaps: Vec<_> = (1..=5)
        .map(|i| snap(&format!("bg_{i}"), &format!("t{i}")))
        .collect();
    assert_eq!(bg_tasks_panel::bg_panel_height(&snaps), 3);
}

#[test]
fn running_task_ids_sorted() {
    let snaps = vec![
        snap("bg_3", "three"),
        snap("bg_1", "one"),
        snap("bg_2", "two"),
    ];
    assert_eq!(
        bg_tasks_panel::running_task_ids(&snaps),
        vec!["bg_3", "bg_1", "bg_2"],
    );
}

// === cycle_bg_task_focus via cycle_panel_focus ===

#[test]
fn forward_through_bg_tasks() {
    let mut app = make_app();
    add_bg_task(&app, "bg_1", "one");
    add_bg_task(&app, "bg_2", "two");
    add_bg_task(&app, "bg_3", "three");
    sync_bg(&mut app);
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);
    cycle_panel_focus(&mut app, true);
    assert_eq!(app.focused_bg_task.as_deref(), Some("bg_1"));
    cycle_panel_focus(&mut app, true);
    assert_eq!(app.focused_bg_task.as_deref(), Some("bg_2"));
    cycle_panel_focus(&mut app, true);
    assert_eq!(app.focused_bg_task.as_deref(), Some("bg_3"));
}

#[test]
fn forward_wraps_around() {
    let mut app = make_app();
    add_bg_task(&app, "bg_1", "one");
    add_bg_task(&app, "bg_2", "two");
    sync_bg(&mut app);
    app.focused_bg_task = Some("bg_2".into());
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);
    cycle_panel_focus(&mut app, true);
    assert_eq!(app.focused_bg_task.as_deref(), Some("bg_1"));
}

#[test]
fn backward_wraps_around() {
    let mut app = make_app();
    add_bg_task(&app, "bg_1", "one");
    add_bg_task(&app, "bg_2", "two");
    sync_bg(&mut app);
    app.focused_bg_task = Some("bg_1".into());
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);
    cycle_panel_focus(&mut app, false);
    assert_eq!(app.focused_bg_task.as_deref(), Some("bg_2"));
}

#[test]
fn backward_from_none_selects_last() {
    let mut app = make_app();
    add_bg_task(&app, "bg_1", "one");
    add_bg_task(&app, "bg_2", "two");
    sync_bg(&mut app);
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);
    cycle_panel_focus(&mut app, false);
    assert_eq!(app.focused_bg_task.as_deref(), Some("bg_2"));
}

#[test]
fn empty_tasks_clears_focus_and_exits_panel() {
    let mut app = make_app();
    app.focused_bg_task = Some("stale".into());
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);
    cycle_panel_focus(&mut app, true);
    assert!(app.focused_bg_task.is_none());
    assert_eq!(
        app.focus_mode,
        FocusMode::Input,
        "should exit panel when bg tasks empty"
    );
}

#[test]
fn stale_focus_recovery() {
    let mut app = make_app();
    add_bg_task(&app, "bg_live", "alive");
    let handle = app.bg_store.register_proxy("bg_done".into(), "done".into());
    handle.complete("output".into(), true);
    sync_bg(&mut app);
    app.focused_bg_task = Some("bg_done".into());
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);
    cycle_panel_focus(&mut app, true);
    assert_eq!(app.focused_bg_task.as_deref(), Some("bg_live"));
}
