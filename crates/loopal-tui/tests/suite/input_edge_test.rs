/// Edge-case input tests: sub-page, modal Ctrl+C, and question dialogs.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::{ControlCommand, Question, QuestionOption, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;

use loopal_tui::input::{InputAction, handle_key};
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

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

// --- Modal Ctrl+C ---

#[test]
fn test_ctrl_c_denies_permission() {
    let mut app = make_app();
    {
        let mut state = app.session.lock();
        state.pending_permission = Some(loopal_session::types::PendingPermission {
            id: "1".into(),
            name: "Bash".into(),
            input: "rm".into(),
            relay_request_id: None,
        });
    }
    let action = handle_key(&mut app, ctrl('c'));
    assert!(matches!(action, InputAction::ToolDeny));
}

#[test]
fn test_ctrl_c_cancels_question() {
    let mut app = make_app();
    {
        let mut state = app.session.lock();
        state.pending_question = Some(loopal_session::types::PendingQuestion::new(
            "q1".into(),
            vec![Question {
                question: "Pick one".into(),
                options: vec![
                    QuestionOption {
                        label: "A".into(),
                        description: "Option A".into(),
                    },
                    QuestionOption {
                        label: "B".into(),
                        description: "Option B".into(),
                    },
                ],
                allow_multiple: false,
            }],
        ));
    }
    let action = handle_key(&mut app, ctrl('c'));
    assert!(matches!(action, InputAction::QuestionCancel));
}

// --- ESC / Ctrl+C on sub-page ---

#[test]
fn test_ctrl_c_closes_sub_page() {
    let mut app = make_app();
    app.last_esc_time = Some(std::time::Instant::now());
    app.sub_page = Some(loopal_tui::app::SubPage::RewindPicker(
        loopal_tui::app::RewindPickerState {
            turns: vec![loopal_tui::app::RewindTurnItem {
                turn_index: 0,
                preview: "test".into(),
            }],
            selected: 0,
        },
    ));
    let action = handle_key(&mut app, ctrl('c'));
    assert!(matches!(action, InputAction::None));
    assert!(app.sub_page.is_none(), "sub-page should be closed");
    assert!(app.last_esc_time.is_none(), "esc time should be cleared");
}

#[test]
fn test_esc_time_cleared_on_picker_close() {
    let mut app = make_app();
    app.last_esc_time = Some(std::time::Instant::now());
    app.sub_page = Some(loopal_tui::app::SubPage::RewindPicker(
        loopal_tui::app::RewindPickerState {
            turns: vec![loopal_tui::app::RewindTurnItem {
                turn_index: 0,
                preview: "test".into(),
            }],
            selected: 0,
        },
    ));
    let _action = handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.sub_page.is_none(), "picker should be closed");
    assert!(app.last_esc_time.is_none(), "esc time should be cleared");
}
