/// Tests for scroll behavior: mouse scroll events vs keyboard arrow keys.
///
/// Mouse scroll uses dedicated `AppEvent::ScrollUp/Down` (via `EnableMouseCapture`).
/// Keyboard Up/Down always routes to history navigation through `handle_key`.
/// The two paths are completely independent — no heuristic detection needed.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::ControlCommand;
use loopal_protocol::UserQuestionResponse;
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

// --- Mouse scroll (ContentScroll direct manipulation) ---

#[test]
fn test_scroll_up_increases_offset() {
    let mut app = make_app();
    app.input_history.push("should not appear".into());
    // Mouse scroll events directly modify content_scroll, never touching history.
    app.content_scroll.scroll_up(3);
    app.content_scroll.scroll_up(3);
    app.content_scroll.scroll_up(3);
    assert_eq!(app.content_scroll.offset, 9, "3 scroll_up(3) should add 9");
    assert!(app.input.is_empty(), "scroll should NOT navigate history");
}

#[test]
fn test_scroll_down_decreases_offset() {
    let mut app = make_app();
    app.content_scroll.offset = 30;
    app.content_scroll.scroll_down(3);
    app.content_scroll.scroll_down(3);
    assert_eq!(
        app.content_scroll.offset, 24,
        "2 scroll_down(3) should reduce by 6"
    );
}

#[test]
fn test_scroll_down_clamps_at_zero() {
    let mut app = make_app();
    app.content_scroll.offset = 2;
    app.content_scroll.scroll_down(3);
    app.content_scroll.scroll_down(3);
    app.content_scroll.scroll_down(3);
    assert_eq!(
        app.content_scroll.offset, 0,
        "scroll_offset should not go negative"
    );
}

// --- Keyboard Up/Down → history ---

#[test]
fn test_up_navigates_history() {
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

// --- Scroll does not affect history ---

#[test]
fn test_scroll_does_not_touch_history() {
    let mut app = make_app();
    app.input_history.push("preserved".into());
    app.content_scroll.scroll_up(3);
    app.content_scroll.scroll_up(3);
    assert!(app.content_scroll.offset > 0);
    assert!(app.input.is_empty(), "scroll must not touch input");
    assert!(
        app.history_index.is_none(),
        "scroll must not set history_index"
    );
}

// --- History does not reset scroll_offset ---

#[test]
fn test_history_navigation_preserves_scroll() {
    let mut app = make_app();
    app.content_scroll.offset = 10;
    app.input_history.push("cmd".into());
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.input, "cmd");
    assert_eq!(
        app.content_scroll.offset, 10,
        "history nav should not reset scroll"
    );
}

// --- Typing resets scroll ---

#[test]
fn test_typing_resets_scroll_offset() {
    let mut app = make_app();
    app.content_scroll.offset = 15;
    handle_key(&mut app, key(KeyCode::Char('x')));
    assert_eq!(
        app.content_scroll.offset, 0,
        "typing should reset scroll to bottom"
    );
}

// --- PageUp/PageDown ---

#[test]
fn test_page_up_down_scroll() {
    let mut app = make_app();
    handle_key(&mut app, key(KeyCode::PageUp));
    assert_eq!(app.content_scroll.offset, 10);
    handle_key(&mut app, key(KeyCode::PageDown));
    assert_eq!(app.content_scroll.offset, 0);
}
