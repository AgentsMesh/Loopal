/// Edge-case tests for Up/Down key scroll reset, agent-busy behavior,
/// Ctrl+P/N scroll reset, and multiline boundary fall-through to history.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::UserQuestionResponse;
use loopal_protocol::{AgentStatus, ControlCommand};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::input::handle_key;
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

// --- Auto-reset scroll: per-key verification ---

#[test]
fn test_backspace_resets_scroll_offset() {
    let mut app = make_app();
    app.input = "ab".into();
    app.input_cursor = 2;
    app.content_scroll.offset = 4;
    handle_key(&mut app, key(KeyCode::Backspace));
    assert_eq!(
        app.content_scroll.offset, 0,
        "Backspace should reset scroll"
    );
    assert_eq!(app.input, "a");
}

#[test]
fn test_cursor_move_resets_scroll_offset() {
    let mut app = make_app();
    app.input = "hello".into();
    app.input_cursor = 3;
    app.content_scroll.offset = 6;
    handle_key(&mut app, key(KeyCode::Left));
    assert_eq!(app.content_scroll.offset, 0, "Left should reset scroll");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_tab_preserves_scroll_offset() {
    let mut app = make_app();
    app.content_scroll.offset = 7;
    handle_key(&mut app, key(KeyCode::Tab));
    assert_eq!(app.content_scroll.offset, 7, "Tab should not reset scroll");
}

#[test]
fn test_esc_preserves_scroll_offset() {
    let mut app = make_app();
    app.content_scroll.offset = 3;
    handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(app.content_scroll.offset, 3, "Esc should not reset scroll");
}

// --- Agent busy: Up/Down still navigates history ---

#[test]
fn test_up_navigates_history_when_agent_busy() {
    let mut app = make_app();
    app.session
        .lock()
        .agents
        .get_mut("main")
        .unwrap()
        .observable
        .status = AgentStatus::Running;
    app.input_history.push("prev".into());
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(
        app.input, "prev",
        "Up should navigate history even when agent busy"
    );
}

// --- Ctrl+P/N preserves scroll offset (history and scroll are decoupled) ---

#[test]
fn test_ctrl_p_preserves_scroll_offset() {
    let mut app = make_app();
    app.content_scroll.offset = 10;
    app.input_history.push("cmd".into());
    handle_key(&mut app, ctrl('p'));
    assert_eq!(
        app.content_scroll.offset, 10,
        "Ctrl+P should not reset scroll"
    );
    assert_eq!(app.input, "cmd");
}

// --- Multiline boundary: fall through to history ---

#[test]
fn test_up_at_first_line_falls_through_to_history() {
    let mut app = make_app();
    app.input = "line1\nline2".into();
    app.input_cursor = 2; // middle of line1 (already at first visual line)
    app.input_history.push("old command".into());
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(
        app.input, "old command",
        "Up at first line should fall through to history"
    );
}

#[test]
fn test_down_at_last_line_falls_through_to_history() {
    let mut app = make_app();
    // First enter history via Up
    app.input_history.push("first".into());
    app.input_history.push("second".into());
    handle_key(&mut app, key(KeyCode::Up)); // "second"
    handle_key(&mut app, key(KeyCode::Up)); // "first"
    // Now set multiline content and cursor at last line
    app.input = "line1\nline2".into();
    app.input_cursor = app.input.len(); // end of line2
    handle_key(&mut app, key(KeyCode::Down));
    // cursor_down returns None (at last line), falls through to history forward
    assert_eq!(
        app.input, "second",
        "Down at last line should fall through to history"
    );
}
