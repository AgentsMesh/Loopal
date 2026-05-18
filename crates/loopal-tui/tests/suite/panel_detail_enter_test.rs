//! E2E tests for enter_*_view dispatch — verifies the action correctly
//! sets up the matching sub_page state with default values.

use loopal_protocol::{
    BgTaskSnapshot, BgTaskStatus, ControlCommand, CronJobSnapshot, TaskSnapshot,
    TaskSnapshotStatus, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::{App, FocusMode, PanelKind, SubPage};
use loopal_tui::input::InputAction;
use loopal_tui::key_dispatch_for_test::dispatch;

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

fn cron(id: &str) -> CronJobSnapshot {
    CronJobSnapshot {
        id: id.into(),
        cron_expr: "* * * * *".into(),
        prompt: "ping".into(),
        recurring: true,
        created_at_unix_ms: 1_700_000_000_000,
        next_fire_unix_ms: Some(1_700_000_060_000),
        durable: false,
    }
}

fn task(id: &str) -> TaskSnapshot {
    TaskSnapshot {
        id: id.into(),
        subject: "do".into(),
        active_form: None,
        status: TaskSnapshotStatus::InProgress,
        blocked_by: Vec::new(),
        description: String::new(),
        blocks: Vec::new(),
    }
}

fn bg(id: &str) -> BgTaskSnapshot {
    BgTaskSnapshot {
        id: id.into(),
        description: "run".into(),
        status: BgTaskStatus::Running,
        exit_code: None,
        created_at_unix_ms: 0,
    }
}

#[tokio::test]
async fn enter_cron_view_opens_detail_sub_page() {
    let mut app = make_app();
    app.view_clients["main"].inject_crons_for_test(vec![cron("abc12345")]);
    app.section_mut(PanelKind::Crons).focused = Some("abc12345".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Crons);

    dispatch(&mut app, InputAction::EnterCronView).await;

    match &app.sub_page {
        Some(SubPage::CronDetail(s)) => assert_eq!(s.cron_id, "abc12345"),
        _ => panic!("expected CronDetail sub-page"),
    }
    assert_eq!(
        app.focus_mode,
        FocusMode::Input,
        "focus should return to Input after entering sub-page"
    );
}

#[tokio::test]
async fn enter_cron_view_without_focus_is_noop() {
    let mut app = make_app();
    app.section_mut(PanelKind::Crons).focused = None;

    dispatch(&mut app, InputAction::EnterCronView).await;

    assert!(app.sub_page.is_none(), "no focused row → no sub-page");
}

#[tokio::test]
async fn enter_task_view_opens_detail_with_zero_scroll() {
    let mut app = make_app();
    app.view_clients["main"].inject_tasks_for_test(vec![task("7")]);
    app.section_mut(PanelKind::Tasks).focused = Some("7".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Tasks);

    dispatch(&mut app, InputAction::EnterTaskView).await;

    match &app.sub_page {
        Some(SubPage::TaskDetail(s)) => {
            assert_eq!(s.task_id, "7");
            assert_eq!(s.scroll_offset, 0);
        }
        _ => panic!("expected TaskDetail sub-page"),
    }
}

#[tokio::test]
async fn enter_task_view_without_focus_is_noop() {
    let mut app = make_app();
    app.section_mut(PanelKind::Tasks).focused = None;

    dispatch(&mut app, InputAction::EnterTaskView).await;

    assert!(app.sub_page.is_none());
}

#[tokio::test]
async fn enter_bg_task_view_opens_log_with_auto_follow() {
    let mut app = make_app();
    app.view_clients["main"].inject_bg_for_test(vec![bg("bg_5")]);
    app.section_mut(PanelKind::BgTasks).focused = Some("bg_5".into());
    app.focus_mode = FocusMode::Panel(PanelKind::BgTasks);

    dispatch(&mut app, InputAction::EnterBgTaskView).await;

    match &app.sub_page {
        Some(SubPage::BgTaskLog(s)) => {
            assert_eq!(s.task_id, "bg_5");
            assert!(s.auto_follow, "log viewer enters in auto-follow mode");
            assert_eq!(s.scroll_offset, 0);
        }
        _ => panic!("expected BgTaskLog sub-page"),
    }
}

#[tokio::test]
async fn enter_bg_task_view_without_focus_is_noop() {
    let mut app = make_app();
    app.section_mut(PanelKind::BgTasks).focused = None;

    dispatch(&mut app, InputAction::EnterBgTaskView).await;

    assert!(app.sub_page.is_none());
}
