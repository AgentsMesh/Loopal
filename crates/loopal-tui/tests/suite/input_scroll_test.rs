/// Tests for Up/Down key routing, Ctrl+P/N history, and multiline priority.
/// Up/Down now go directly to history navigation (no debounce).
/// Ctrl+P/N always navigate history. Scroll is handled separately via batch detection.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::UserQuestionResponse;
use loopal_protocol::{AgentStatus, ControlCommand};
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

// --- PageUp / PageDown ---

#[test]
fn test_page_up_down_scroll() {
    let mut app = make_app();
    handle_key(&mut app, key(KeyCode::PageUp));
    assert_eq!(app.content_scroll.offset, 10);
    handle_key(&mut app, key(KeyCode::PageDown));
    assert_eq!(app.content_scroll.offset, 0);
}

// --- Up/Down navigate history directly ---

#[test]
fn test_up_navigates_history_with_content() {
    let mut app = make_app();
    app.input_history.push("previous".into());
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.input, "previous", "Up should browse history");
}

#[test]
fn test_down_navigates_history_forward() {
    let mut app = make_app();
    app.input_history.push("first".into());
    app.input_history.push("second".into());
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.input, "first");
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.input, "second", "Down should navigate history forward");
}

#[test]
fn test_up_navigates_history_when_idle() {
    let mut app = make_app();
    app.session
        .lock()
        .agents
        .get_mut("main")
        .unwrap()
        .observable
        .status = AgentStatus::WaitingForInput;
    app.input_history.push("older".into());
    app.input_history.push("recent".into());
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.input, "recent", "Up should browse history");
}

#[test]
fn test_down_navigates_history_forward_when_idle() {
    let mut app = make_app();
    app.session
        .lock()
        .agents
        .get_mut("main")
        .unwrap()
        .observable
        .status = AgentStatus::WaitingForInput;
    app.input_history.push("first".into());
    app.input_history.push("second".into());
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.input, "first");
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.input, "second", "Down should navigate history forward");
}

#[test]
fn test_up_navigates_history_immediately() {
    let mut app = make_app();
    app.session
        .lock()
        .agents
        .get_mut("main")
        .unwrap()
        .observable
        .status = AgentStatus::WaitingForInput;
    app.input_history.push("previous command".into());
    let action = handle_key(&mut app, key(KeyCode::Up));
    assert!(matches!(action, InputAction::None));
    assert_eq!(
        app.input, "previous command",
        "Up browses history immediately"
    );
}

// --- Ctrl+P/N history navigation ---

#[test]
fn test_ctrl_p_navigates_history() {
    let mut app = make_app();
    app.session
        .lock()
        .agents
        .get_mut("main")
        .unwrap()
        .observable
        .status = AgentStatus::WaitingForInput;
    app.input_history.push("first".into());
    app.input_history.push("second".into());
    handle_key(&mut app, ctrl('p'));
    assert_eq!(app.input, "second", "Ctrl+P should browse history");
}

#[test]
fn test_ctrl_n_navigates_history_forward() {
    let mut app = make_app();
    app.session
        .lock()
        .agents
        .get_mut("main")
        .unwrap()
        .observable
        .status = AgentStatus::WaitingForInput;
    app.input_history.push("first".into());
    app.input_history.push("second".into());
    handle_key(&mut app, ctrl('p'));
    handle_key(&mut app, ctrl('p'));
    assert_eq!(app.input, "first");
    handle_key(&mut app, ctrl('n'));
    assert_eq!(app.input, "second", "Ctrl+N browses history forward");
}

// --- Multiline cursor priority ---

#[test]
fn test_up_multiline_cursor_beats_history() {
    let mut app = make_app();
    app.input = "line1\nline2".into();
    app.input_cursor = app.input.len();
    let action = handle_key(&mut app, key(KeyCode::Up));
    assert!(matches!(action, InputAction::None));
    assert!(
        app.input_cursor < "line1\n".len(),
        "cursor should be on line1, got {}",
        app.input_cursor
    );
}

#[test]
fn test_down_multiline_cursor_beats_history() {
    let mut app = make_app();
    app.input = "line1\nline2".into();
    app.input_cursor = 0;
    let action = handle_key(&mut app, key(KeyCode::Down));
    assert!(matches!(action, InputAction::None));
    assert!(
        app.input_cursor >= "line1\n".len(),
        "cursor should be on line2, got {}",
        app.input_cursor
    );
}

// --- Auto-reset scroll on input interaction ---

#[test]
fn test_typing_resets_scroll_offset() {
    let mut app = make_app();
    app.content_scroll.offset = 5;
    handle_key(&mut app, key(KeyCode::Char('a')));
    assert_eq!(
        app.content_scroll.offset, 0,
        "typing should reset scroll to bottom"
    );
    assert_eq!(app.input, "a");
}

// --- Up/Down do NOT reset scroll_offset (scroll and history are decoupled) ---

#[test]
fn test_up_preserves_scroll_offset() {
    let mut app = make_app();
    app.content_scroll.offset = 10;
    app.input_history.push("cmd".into());
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.input, "cmd");
    assert_eq!(
        app.content_scroll.offset, 10,
        "Up should not reset scroll_offset"
    );
}
