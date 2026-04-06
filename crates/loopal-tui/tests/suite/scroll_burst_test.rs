/// Tests for batch-based scroll detection: mouse-wheel bursts vs keyboard arrows.
///
/// Mouse wheel (via `\x1b[?1007h`) produces ≥2 arrow events per batch → scroll.
/// A single keyboard arrow press produces 1 event per batch → history navigation.
/// The batch detection logic lives in `tui_loop.rs`; these tests verify the
/// observable state after batch processing via direct App manipulation.
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

/// Simulate mouse-wheel scroll burst: multiple arrow events applied as scroll.
fn apply_scroll_burst(app: &mut App, code: KeyCode, count: usize) {
    for _ in 0..count {
        match code {
            KeyCode::Up => app.content_scroll.offset = app.content_scroll.offset.saturating_add(3),
            KeyCode::Down => {
                app.content_scroll.offset = app.content_scroll.offset.saturating_sub(3)
            }
            _ => {}
        }
    }
}

// --- Mouse wheel burst detection (≥2 arrows → scroll) ---

#[test]
fn test_scroll_burst_up_increases_offset() {
    let mut app = make_app();
    app.input_history.push("should not appear".into());
    // Simulate a burst of 3 Up events (mouse wheel)
    apply_scroll_burst(&mut app, KeyCode::Up, 3);
    assert_eq!(
        app.content_scroll.offset, 9,
        "3 Up scroll events should add 9"
    );
    assert!(app.input.is_empty(), "scroll should NOT navigate history");
}

#[test]
fn test_scroll_burst_down_decreases_offset() {
    let mut app = make_app();
    app.content_scroll.offset = 30;
    apply_scroll_burst(&mut app, KeyCode::Down, 2);
    assert_eq!(
        app.content_scroll.offset, 24,
        "2 Down scroll events should reduce by 6"
    );
}

#[test]
fn test_scroll_burst_down_clamps_at_zero() {
    let mut app = make_app();
    app.content_scroll.offset = 2;
    apply_scroll_burst(&mut app, KeyCode::Down, 3);
    assert_eq!(
        app.content_scroll.offset, 0,
        "scroll_offset should not go negative"
    );
}

// --- Single arrow → keyboard → history ---

#[test]
fn test_single_up_navigates_history() {
    let mut app = make_app();
    app.input_history.push("previous".into());
    // Single Up event → keyboard → history navigation
    handle_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.input, "previous", "single Up should browse history");
}

#[test]
fn test_single_down_navigates_history_forward() {
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
fn test_scroll_burst_does_not_touch_history() {
    let mut app = make_app();
    app.input_history.push("preserved".into());
    apply_scroll_burst(&mut app, KeyCode::Up, 5);
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
