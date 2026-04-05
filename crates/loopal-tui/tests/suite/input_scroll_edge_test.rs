/// Edge-case tests for Up/Down key scroll reset, agent-busy behavior,
/// Ctrl+P/N scroll reset, and multiline boundary fall-through to history.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::ControlCommand;
use loopal_protocol::UserQuestionResponse;
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::input::{handle_key, resolve_arrow_debounce};
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

fn arrow(app: &mut App, code: KeyCode) {
    handle_key(app, key(code));
    resolve_arrow_debounce(app);
}

// --- Auto-reset scroll: per-key verification ---

#[test]
fn test_backspace_resets_scroll_offset() {
    let mut app = make_app();
    app.input = "ab".into();
    app.input_cursor = 2;
    app.scroll_offset = 4;
    handle_key(&mut app, key(KeyCode::Backspace));
    assert_eq!(app.scroll_offset, 0, "Backspace should reset scroll");
    assert_eq!(app.input, "a");
}

#[test]
fn test_cursor_move_resets_scroll_offset() {
    let mut app = make_app();
    app.input = "hello".into();
    app.input_cursor = 3;
    app.scroll_offset = 6;
    handle_key(&mut app, key(KeyCode::Left));
    assert_eq!(app.scroll_offset, 0, "Left should reset scroll");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_tab_preserves_scroll_offset() {
    let mut app = make_app();
    app.scroll_offset = 7;
    handle_key(&mut app, key(KeyCode::Tab));
    assert_eq!(app.scroll_offset, 7, "Tab should not reset scroll");
}

#[test]
fn test_esc_preserves_scroll_offset() {
    let mut app = make_app();
    app.scroll_offset = 3;
    handle_key(&mut app, key(KeyCode::Esc));
    assert_eq!(app.scroll_offset, 3, "Esc should not reset scroll");
}

// --- Agent busy: Up/Down still navigates history (after debounce) ---

#[test]
fn test_up_navigates_history_when_agent_busy() {
    let mut app = make_app();
    app.session.lock().active_conversation_mut().agent_idle = false;
    app.input_history.push("prev".into());
    arrow(&mut app, KeyCode::Up);
    assert_eq!(
        app.input, "prev",
        "Up should navigate history even when agent busy"
    );
}

// --- Ctrl+P/N resets scroll offset ---

#[test]
fn test_ctrl_p_resets_scroll_offset() {
    let mut app = make_app();
    app.scroll_offset = 10;
    app.input_history.push("cmd".into());
    handle_key(&mut app, ctrl('p'));
    assert_eq!(app.scroll_offset, 0, "Ctrl+P should reset scroll");
    assert_eq!(app.input, "cmd");
}

// --- Multiline boundary: fall through to history ---

#[test]
fn test_up_at_first_line_falls_through_to_history() {
    let mut app = make_app();
    app.input = "line1\nline2".into();
    app.input_cursor = 2; // middle of line1 (already at first visual line)
    app.input_history.push("old command".into());
    arrow(&mut app, KeyCode::Up);
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
    arrow(&mut app, KeyCode::Up); // "second"
    arrow(&mut app, KeyCode::Up); // "first"
    // Now set multiline content and cursor at last line
    app.input = "line1\nline2".into();
    app.input_cursor = app.input.len(); // end of line2
    arrow(&mut app, KeyCode::Down);
    // cursor_down returns None (at last line), falls through to history forward
    assert_eq!(
        app.input, "second",
        "Down at last line should fall through to history"
    );
}

// --- Debounce resolution on non-arrow key ---

#[test]
fn test_typing_after_up_resolves_pending_as_history() {
    let mut app = make_app();
    app.input_history.push("hist".into());
    // Press Up — starts debounce
    handle_key(&mut app, key(KeyCode::Up));
    assert!(app.input.is_empty(), "Up is pending, not yet resolved");
    // Type 'x' — resolves the pending Up as history, then inserts 'x'
    handle_key(&mut app, key(KeyCode::Char('x')));
    assert_eq!(
        app.input, "histx",
        "pending Up resolves as history, then 'x' appends"
    );
}

// --- Global shortcut discards pending debounce ---

#[test]
fn test_ctrl_c_discards_pending_debounce() {
    let mut app = make_app();
    app.input = "some text".into();
    app.input_cursor = 9;
    app.input_history.push("hist".into());
    // Press Up — starts debounce
    handle_key(&mut app, key(KeyCode::Up));
    // Ctrl+C — clears input AND discards pending debounce
    handle_key(&mut app, ctrl('c'));
    assert!(app.input.is_empty(), "Ctrl+C should clear input");
    // Stale timer fires — should be a no-op, not load history
    resolve_arrow_debounce(&mut app);
    assert!(
        app.input.is_empty(),
        "stale timer after Ctrl+C must not load history"
    );
}
