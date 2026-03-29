/// Input handling tests: key routing priority chain + basic interactions.
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

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

// --- Ctrl+C three-level behavior ---

#[test]
fn test_ctrl_c_clears_input_when_non_empty() {
    let mut app = make_app();
    app.input = "hello".to_string();
    app.input_cursor = 5;
    app.history_index = Some(2);
    let action = handle_key(&mut app, ctrl('c'));
    assert!(matches!(action, InputAction::None));
    assert!(app.input.is_empty());
    assert_eq!(app.input_cursor, 0);
    assert!(app.history_index.is_none(), "history_index should be reset");
}

#[test]
fn test_ctrl_c_interrupts_when_agent_busy() {
    let mut app = make_app();
    app.session.lock().agent_idle = false;
    let action = handle_key(&mut app, ctrl('c'));
    assert!(matches!(action, InputAction::Interrupt));
}

#[test]
fn test_ctrl_c_noop_when_idle_and_empty() {
    let mut app = make_app();
    app.session.lock().agent_idle = true;
    let action = handle_key(&mut app, ctrl('c'));
    assert!(matches!(action, InputAction::None));
}

// --- Global shortcuts ---

#[test]
fn test_ctrl_d_quits() {
    let mut app = make_app();
    let action = handle_key(&mut app, ctrl('d'));
    assert!(matches!(action, InputAction::Quit));
}

#[test]
fn test_shift_tab_toggles_mode() {
    let mut app = make_app();
    let action = handle_key(
        &mut app,
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
    );
    assert!(matches!(action, InputAction::ModeSwitch(m) if m == "plan"));
}

// --- Tool confirm takes priority ---

#[test]
fn test_tool_confirm_y_approves() {
    let mut app = make_app();
    {
        let mut state = app.session.lock();
        state.pending_permission = Some(loopal_session::types::PendingPermission {
            id: "1".into(),
            name: "Bash".into(),
            input: "ls".into(),
            relay_request_id: None,
        });
    }
    let action = handle_key(&mut app, key(KeyCode::Char('y')));
    assert!(matches!(action, InputAction::ToolApprove));
}

#[test]
fn test_tool_confirm_n_denies() {
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
    let action = handle_key(&mut app, key(KeyCode::Char('n')));
    assert!(matches!(action, InputAction::ToolDeny));
}

#[test]
fn test_tool_confirm_esc_denies() {
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
    let action = handle_key(&mut app, key(KeyCode::Esc));
    assert!(matches!(action, InputAction::ToolDeny));
}

// --- Character insertion ---

#[test]
fn test_char_inserts_into_input() {
    let mut app = make_app();
    handle_key(&mut app, key(KeyCode::Char('h')));
    handle_key(&mut app, key(KeyCode::Char('i')));
    assert_eq!(app.input, "hi");
    assert_eq!(app.input_cursor, 2);
}

// --- Enter submits ---

#[test]
fn test_enter_submits_text() {
    let mut app = make_app();
    app.input = "hello".to_string();
    app.input_cursor = 5;
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::InboxPush(t) if t.text == "hello"));
    assert!(app.input.is_empty());
}

#[test]
fn test_enter_empty_does_nothing() {
    let mut app = make_app();
    let action = handle_key(&mut app, key(KeyCode::Enter));
    assert!(matches!(action, InputAction::None));
}

// --- Backspace ---

#[test]
fn test_backspace_deletes_character() {
    let mut app = make_app();
    app.input = "abc".to_string();
    app.input_cursor = 3;
    handle_key(&mut app, key(KeyCode::Backspace));
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 2);
}

// --- Cursor navigation ---

#[test]
fn test_home_end_navigation() {
    let mut app = make_app();
    app.input = "hello".to_string();
    app.input_cursor = 3;
    handle_key(&mut app, key(KeyCode::Home));
    assert_eq!(app.input_cursor, 0);
    handle_key(&mut app, key(KeyCode::End));
    assert_eq!(app.input_cursor, 5);
}
