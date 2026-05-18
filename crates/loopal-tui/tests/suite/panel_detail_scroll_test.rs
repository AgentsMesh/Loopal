//! Scroll model tests for sub-pages: TaskDetail handles ↑/↓/PgUp/PgDn;
//! CronDetail has no scroll model and must noop those keys.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, CronDetailState, SubPage, TaskDetailState};
use loopal_tui::input::{InputAction, handle_key};

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

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn task_detail_page_down_advances_by_ten() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::TaskDetail(TaskDetailState {
        task_id: "1".into(),
        scroll_offset: 5,
    }));
    let _ = handle_key(&mut app, key(KeyCode::PageDown));
    if let Some(SubPage::TaskDetail(s)) = &app.sub_page {
        assert_eq!(s.scroll_offset, 15);
    } else {
        panic!("expected TaskDetail sub-page");
    }
}

#[test]
fn task_detail_page_up_saturates_to_zero() {
    let mut app = make_app();
    app.sub_page = Some(SubPage::TaskDetail(TaskDetailState {
        task_id: "1".into(),
        scroll_offset: 3,
    }));
    let _ = handle_key(&mut app, key(KeyCode::PageUp));
    if let Some(SubPage::TaskDetail(s)) = &app.sub_page {
        assert_eq!(s.scroll_offset, 0, "PageUp from 3 with step 10 saturates");
    } else {
        panic!("expected TaskDetail sub-page");
    }
}

#[test]
fn cron_detail_arrow_keys_are_noop() {
    // Cron detail has no scroll model — only Esc / x. Defensive check
    // that arrow keys don't accidentally trigger anything.
    let mut app = make_app();
    app.sub_page = Some(SubPage::CronDetail(CronDetailState {
        cron_id: "abc12345".into(),
    }));
    for code in [
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::PageUp,
        KeyCode::PageDown,
    ] {
        let action = handle_key(&mut app, key(code));
        assert!(
            matches!(action, InputAction::None),
            "{code:?} should be noop in CronDetail"
        );
    }
    assert!(matches!(app.sub_page, Some(SubPage::CronDetail(_))));
}
