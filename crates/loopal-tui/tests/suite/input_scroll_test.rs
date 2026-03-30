/// Tests for Up/Down scroll routing, Ctrl+P/N history, and multiline priority.
///
/// Priority chain for Up/Down (via xterm alternate scroll = mouse wheel):
///   1. Multiline cursor navigation (Shift+Enter input)
///   2. Content scroll
///   3. History navigation (only when content fits)
///
/// Ctrl+P/N always navigate history regardless of scroll state.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::ControlCommand;
use loopal_protocol::UserQuestionResponse;
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
    assert_eq!(app.scroll_offset, 10);
    handle_key(&mut app, key(KeyCode::PageDown));
    assert_eq!(app.scroll_offset, 0);
}

// --- Up/Down content scroll ---

#[test]
fn test_up_scrolls_when_content_overflows() {
    let mut app = make_app();
    app.content_overflows = true;
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.scroll_offset, 1, "Up should scroll +1 when overflows");
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.scroll_offset, 2, "repeated Up keeps incrementing");
}

#[test]
fn test_down_scrolls_back_when_offset_positive() {
    let mut app = make_app();
    app.scroll_offset = 5;
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.scroll_offset, 4, "Down should scroll -1 when offset>0");
}

#[test]
fn test_up_scrolls_when_content_overflows_and_idle() {
    let mut app = make_app();
    app.session.lock().active_conversation_mut().agent_idle = true;
    app.content_overflows = true;
    app.input_history.push("older".into());
    app.input_history.push("recent".into());
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.scroll_offset, 1, "Up should scroll even when idle");
    assert!(app.input.is_empty(), "input stays empty (no history)");
}

#[test]
fn test_down_absorbed_when_content_overflows_at_bottom() {
    let mut app = make_app();
    app.session.lock().active_conversation_mut().agent_idle = true;
    app.content_overflows = true;
    app.input_history.push("cmd".into());
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.scroll_offset, 0, "scroll_offset stays 0");
    assert!(
        app.input.is_empty(),
        "Down at bottom should not trigger history"
    );
}

// --- Up/Down history (content fits) ---

#[test]
fn test_up_navigates_history_when_content_fits() {
    let mut app = make_app();
    app.session.lock().active_conversation_mut().agent_idle = true;
    app.content_overflows = false;
    app.input_history.push("previous command".into());
    let action = handle_key(&mut app, key(KeyCode::Up));
    assert!(matches!(action, InputAction::None));
    assert_eq!(app.input, "previous command", "Up browses history");
    assert_eq!(app.scroll_offset, 0);
}

// --- Ctrl+P/N history navigation ---

#[test]
fn test_ctrl_p_navigates_history_when_content_overflows() {
    let mut app = make_app();
    app.session.lock().active_conversation_mut().agent_idle = true;
    app.content_overflows = true;
    app.input_history.push("first".into());
    app.input_history.push("second".into());
    handle_key(&mut app, ctrl('p'));
    assert_eq!(app.input, "second", "Ctrl+P should browse history");
    assert_eq!(app.scroll_offset, 0, "Ctrl+P should not scroll");
}

#[test]
fn test_ctrl_n_navigates_history_forward() {
    let mut app = make_app();
    app.session.lock().active_conversation_mut().agent_idle = true;
    app.input_history.push("first".into());
    app.input_history.push("second".into());
    handle_key(&mut app, ctrl('p'));
    handle_key(&mut app, ctrl('p'));
    assert_eq!(app.input, "first");
    handle_key(&mut app, ctrl('n'));
    assert_eq!(app.input, "second", "Ctrl+N browses history forward");
}

// --- Multiline cursor priority over scroll ---

#[test]
fn test_up_multiline_cursor_beats_scroll() {
    let mut app = make_app();
    app.session.lock().active_conversation_mut().agent_idle = true;
    app.content_overflows = true;
    app.input = "line1\nline2".into();
    app.input_cursor = app.input.len(); // end of line2
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.scroll_offset, 0, "should move cursor, not scroll");
    assert!(
        app.input_cursor < "line1\n".len(),
        "cursor should be on line1, got {}",
        app.input_cursor
    );
}

#[test]
fn test_down_multiline_cursor_beats_absorb() {
    let mut app = make_app();
    app.session.lock().active_conversation_mut().agent_idle = true;
    app.content_overflows = true;
    app.input = "line1\nline2".into();
    app.input_cursor = 0; // start of line1
    handle_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.scroll_offset, 0, "should move cursor, not scroll");
    assert!(
        app.input_cursor >= "line1\n".len(),
        "cursor should be on line2, got {}",
        app.input_cursor
    );
}
