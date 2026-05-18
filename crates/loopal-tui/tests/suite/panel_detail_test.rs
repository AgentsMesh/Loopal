use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::{
    BgTaskSnapshot, BgTaskStatus, ControlCommand, CronJobSnapshot, TaskSnapshot,
    TaskSnapshotStatus, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::{
    App, BgTaskLogState, CronDetailState, FocusMode, PanelKind, SubPage, TaskDetailState,
};
use loopal_tui::input::{InputAction, handle_key};

use tokio::sync::mpsc;

fn make_app() -> (App, mpsc::Receiver<ControlCommand>) {
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    (App::new(session, std::env::temp_dir()), control_rx)
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
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

#[test]
fn enter_in_crons_panel_emits_enter_cron_view() {
    let (mut app, _rx) = make_app();
    app.view_clients["main"].inject_crons_for_test(vec![cron("abc12345")]);
    app.section_mut(PanelKind::Crons).focused = Some("abc12345".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Crons);
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::EnterCronView));
}

#[test]
fn enter_in_tasks_panel_emits_enter_task_view() {
    let (mut app, _rx) = make_app();
    app.view_clients["main"].inject_tasks_for_test(vec![task("1")]);
    app.section_mut(PanelKind::Tasks).focused = Some("1".into());
    app.focus_mode = FocusMode::Panel(PanelKind::Tasks);
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::EnterTaskView));
}

#[test]
fn x_in_bg_task_log_emits_stop_action() {
    let (mut app, _rx) = make_app();
    app.sub_page = Some(SubPage::BgTaskLog(BgTaskLogState {
        task_id: "bg_1".into(),
        scroll_offset: 0,
        auto_follow: true,
        prev_line_count: 0,
    }));
    let action = handle_key(&mut app, key(KeyCode::Char('x')));
    assert!(matches!(action, InputAction::StopFocusedSubPageItem));
}

#[test]
fn x_in_cron_detail_emits_stop_action() {
    let (mut app, _rx) = make_app();
    app.sub_page = Some(SubPage::CronDetail(CronDetailState {
        cron_id: "abc12345".into(),
    }));
    let action = handle_key(&mut app, key(KeyCode::Char('x')));
    assert!(matches!(action, InputAction::StopFocusedSubPageItem));
}

#[test]
fn x_in_task_detail_is_noop() {
    let (mut app, _rx) = make_app();
    app.sub_page = Some(SubPage::TaskDetail(TaskDetailState {
        task_id: "1".into(),
        scroll_offset: 0,
    }));
    let action = handle_key(&mut app, key(KeyCode::Char('x')));
    assert!(matches!(action, InputAction::None));
}

#[test]
fn esc_in_cron_detail_closes_sub_page() {
    let (mut app, _rx) = make_app();
    app.sub_page = Some(SubPage::CronDetail(CronDetailState {
        cron_id: "abc12345".into(),
    }));
    let _ = handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.sub_page.is_none());
}

#[test]
fn esc_in_task_detail_closes_sub_page() {
    let (mut app, _rx) = make_app();
    app.sub_page = Some(SubPage::TaskDetail(TaskDetailState {
        task_id: "1".into(),
        scroll_offset: 5,
    }));
    let _ = handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.sub_page.is_none());
}

#[test]
fn task_detail_scroll_down_advances_offset() {
    let (mut app, _rx) = make_app();
    app.sub_page = Some(SubPage::TaskDetail(TaskDetailState {
        task_id: "1".into(),
        scroll_offset: 0,
    }));
    let _ = handle_key(&mut app, key(KeyCode::Down));
    if let Some(SubPage::TaskDetail(s)) = &app.sub_page {
        assert_eq!(s.scroll_offset, 1);
    } else {
        panic!("expected TaskDetail sub-page");
    }
}

#[test]
fn task_detail_scroll_up_saturates() {
    let (mut app, _rx) = make_app();
    app.sub_page = Some(SubPage::TaskDetail(TaskDetailState {
        task_id: "1".into(),
        scroll_offset: 0,
    }));
    let _ = handle_key(&mut app, key(KeyCode::Up));
    if let Some(SubPage::TaskDetail(s)) = &app.sub_page {
        assert_eq!(s.scroll_offset, 0);
    } else {
        panic!("expected TaskDetail sub-page");
    }
}

#[test]
fn bg_panel_x_silenced_when_not_focus_panel() {
    let (mut app, _rx) = make_app();
    app.view_clients["main"].inject_bg_for_test(vec![bg("bg_1")]);
    app.section_mut(PanelKind::BgTasks).focused = Some("bg_1".into());
    app.focus_mode = FocusMode::Input;
    let action = handle_key(&mut app, key(KeyCode::Char('x')));
    assert!(matches!(action, InputAction::None));
}
