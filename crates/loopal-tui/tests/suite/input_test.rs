/// Input handling tests: key routing priority chain + basic interactions.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::command::builtin_entries;
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
    App::new(session, builtin_entries(), std::env::temp_dir())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

// --- Priority chain ---

#[test]
fn test_ctrl_c_quits() {
    let mut app = make_app();
    let action = handle_key(&mut app, ctrl('c'));
    assert!(matches!(action, InputAction::Quit));
}

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
    // Simulate pending permission
    {
        let mut state = app.session.lock();
        state.pending_permission = Some(loopal_session::types::PendingPermission {
            id: "1".into(),
            name: "Bash".into(),
            input: "ls".into(),
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

// --- Scroll ---

#[test]
fn test_page_up_down_scroll() {
    let mut app = make_app();
    handle_key(&mut app, key(KeyCode::PageUp));
    assert_eq!(app.scroll_offset, 10);
    handle_key(&mut app, key(KeyCode::PageDown));
    assert_eq!(app.scroll_offset, 0);
}

#[test]
fn test_up_scrolls_when_content_overflows() {
    let mut app = make_app();
    app.content_overflows = true;
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.scroll_offset, 1, "Up should scroll +1 when content overflows");
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.scroll_offset, 2, "repeated Up should keep incrementing");
}

#[test]
fn test_down_scrolls_back_when_offset_positive() {
    let mut app = make_app();
    app.scroll_offset = 5;
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.scroll_offset, 4, "Down should scroll -1 when offset > 0");
}

#[test]
fn test_up_navigates_history_when_content_fits() {
    let mut app = make_app();
    app.session.lock().agent_idle = true;
    app.content_overflows = false;
    app.input_history.push("previous command".into());
    let action = handle_key(&mut app, key(KeyCode::Up));
    assert!(matches!(action, InputAction::None));
    assert_eq!(app.input, "previous command", "Up should browse history when content fits");
    assert_eq!(app.scroll_offset, 0, "scroll_offset should stay 0");
}

// --- ESC clears on sub-page close ---

#[test]
fn test_esc_time_cleared_on_picker_close() {
    let mut app = make_app();
    // Set a stale esc time
    app.last_esc_time = Some(std::time::Instant::now());
    // Open and close a rewind picker via sub_page
    app.sub_page = Some(loopal_tui::app::SubPage::RewindPicker(
        loopal_tui::app::RewindPickerState {
            turns: vec![loopal_tui::app::RewindTurnItem {
                turn_index: 0,
                preview: "test".into(),
            }],
            selected: 0,
        },
    ));
    // Press Esc to close picker
    let _action = handle_key(&mut app, key(KeyCode::Esc));
    assert!(app.sub_page.is_none(), "picker should be closed");
    assert!(app.last_esc_time.is_none(), "esc time should be cleared");
}
