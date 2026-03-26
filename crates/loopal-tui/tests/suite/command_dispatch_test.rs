/// Command dispatch integration: input key → RunCommand → effect.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use loopal_protocol::{ControlCommand, UserQuestionResponse};
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

#[test]
fn test_slash_command_input_produces_run_command() {
    let mut app = make_app();
    app.input = "/clear".to_string();
    app.input_cursor = 6;
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::RunCommand(ref name, None) if name == "/clear"));
}

#[test]
fn test_slash_command_with_arg_produces_run_command() {
    let mut app = make_app();
    app.input = "/help commit".to_string();
    app.input_cursor = 12;
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(
        matches!(action, InputAction::RunCommand(ref name, Some(ref arg)) if name == "/help" && arg == "commit"),
    );
}

#[test]
fn test_unknown_slash_falls_through_as_message() {
    let mut app = make_app();
    app.input = "/nonexistent".to_string();
    app.input_cursor = 12;
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::InboxPush(_)));
}

#[test]
fn test_normal_message_not_treated_as_command() {
    let mut app = make_app();
    app.input = "hello world".to_string();
    app.input_cursor = 11;
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::InboxPush(_)));
}

#[test]
fn test_slash_only_not_treated_as_command() {
    let mut app = make_app();
    app.input = "/".to_string();
    app.input_cursor = 1;
    let action = handle_key(&mut app, key(KeyCode::Enter));
    // "/" alone is not a known command — falls through as message
    assert!(matches!(action, InputAction::InboxPush(_)));
}
