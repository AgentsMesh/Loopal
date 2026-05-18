//! E2E tests for stop_focused_sub_page_item — covers the full
//! action → ControlCommand dispatch + sub_page closure chain.

use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, BgTaskLogState, CronDetailState, SubPage, TaskDetailState};
use loopal_tui::input::InputAction;
use loopal_tui::key_dispatch_for_test::dispatch;

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

#[tokio::test]
async fn stop_in_bg_task_log_sends_kill_and_closes_page() {
    let (mut app, mut rx) = make_app();
    app.sub_page = Some(SubPage::BgTaskLog(BgTaskLogState {
        task_id: "bg_42".into(),
        scroll_offset: 0,
        auto_follow: true,
        prev_line_count: 0,
    }));

    dispatch(&mut app, InputAction::StopFocusedSubPageItem).await;

    let cmd = rx.try_recv().expect("expected ControlCommand on stop");
    match cmd {
        ControlCommand::BgTaskKill { id } => assert_eq!(id, "bg_42"),
        other => panic!("expected BgTaskKill, got {other:?}"),
    }
    assert!(app.sub_page.is_none(), "sub_page must close after stop");
}

#[tokio::test]
async fn stop_in_cron_detail_sends_delete_and_closes_page() {
    let (mut app, mut rx) = make_app();
    app.sub_page = Some(SubPage::CronDetail(CronDetailState {
        cron_id: "abc12345".into(),
    }));

    dispatch(&mut app, InputAction::StopFocusedSubPageItem).await;

    let cmd = rx.try_recv().expect("expected ControlCommand on stop");
    match cmd {
        ControlCommand::CronDelete { id } => assert_eq!(id, "abc12345"),
        other => panic!("expected CronDelete, got {other:?}"),
    }
    assert!(app.sub_page.is_none(), "sub_page must close after stop");
}

#[tokio::test]
async fn stop_in_task_detail_is_noop_and_keeps_page_open() {
    let (mut app, mut rx) = make_app();
    app.sub_page = Some(SubPage::TaskDetail(TaskDetailState {
        task_id: "1".into(),
        scroll_offset: 0,
    }));

    dispatch(&mut app, InputAction::StopFocusedSubPageItem).await;

    // TaskDetail has no stop semantics — neither a ControlCommand fires
    // nor does the page close. Defensive: 'x' in TaskDetail returns None
    // via the key handler, but stop_focused_sub_page_item must remain
    // safe if dispatched erroneously from another code path.
    assert!(
        rx.try_recv().is_err(),
        "no ControlCommand should be sent for TaskDetail"
    );
    assert!(
        matches!(app.sub_page, Some(SubPage::TaskDetail(_))),
        "TaskDetail must stay open when stop is dispatched"
    );
}

#[tokio::test]
async fn stop_with_no_sub_page_is_safe_noop() {
    let (mut app, mut rx) = make_app();
    app.sub_page = None;

    dispatch(&mut app, InputAction::StopFocusedSubPageItem).await;

    assert!(rx.try_recv().is_err());
    assert!(app.sub_page.is_none());
}
