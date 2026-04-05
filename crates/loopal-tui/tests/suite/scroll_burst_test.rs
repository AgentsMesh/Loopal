/// Tests for arrow-key debounce: mouse-wheel burst detection and stale state.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use loopal_protocol::ControlCommand;
use loopal_protocol::UserQuestionResponse;
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::input::{InputAction, handle_key, resolve_arrow_debounce};
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

// --- Mouse wheel burst detection ---

#[test]
fn test_rapid_up_burst_scrolls_content() {
    let mut app = make_app();
    app.input_history.push("should not appear".into());
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Up));
    assert!(app.scroll_offset > 0, "burst should scroll content");
    assert!(app.input.is_empty(), "burst should NOT navigate history");
}

#[test]
fn test_rapid_down_burst_scrolls_content() {
    let mut app = make_app();
    app.scroll_offset = 20;
    handle_key(&mut app, key(KeyCode::Down));
    handle_key(&mut app, key(KeyCode::Down));
    assert!(app.scroll_offset < 20, "burst should scroll down");
}

#[test]
fn test_continuous_scroll_burst() {
    let mut app = make_app();
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Up));
    // 1st+2nd: Pending→Scrolling (scroll 2×3=6), 3rd: continues (+3)
    assert_eq!(app.scroll_offset, 9, "3 rapid Up events should scroll 9");
}

#[test]
fn test_down_burst_exact_offset() {
    let mut app = make_app();
    app.scroll_offset = 30;
    handle_key(&mut app, key(KeyCode::Down));
    handle_key(&mut app, key(KeyCode::Down));
    // Pending→Scrolling: scroll down 2×3=6
    assert_eq!(app.scroll_offset, 24, "2 rapid Down should reduce by 6");
}

// --- Mixed direction burst ---

#[test]
fn test_mixed_direction_burst_applies_both() {
    let mut app = make_app();
    app.scroll_offset = 10;
    handle_key(&mut app, key(KeyCode::Up));   // Pending(Up)
    handle_key(&mut app, key(KeyCode::Down)); // burst: scroll Up+3 then Down-3
    // Net effect: +3 - 3 = 0 change
    assert_eq!(app.scroll_offset, 10, "Up then Down burst should net zero");
}

// --- Stale Scrolling degrades ---

#[test]
fn test_stale_scrolling_degrades_to_debounce() {
    let mut app = make_app();
    app.input_history.push("hist".into());
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Up));
    assert!(app.scroll_offset > 0);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let action = handle_key(&mut app, key(KeyCode::Up));
    assert!(
        matches!(action, InputAction::StartArrowDebounce),
        "stale Scrolling should degrade to new debounce"
    );
    resolve_arrow_debounce(&mut app);
    assert_eq!(app.input, "hist");
}

// --- Empty history + debounce ---

#[test]
fn test_up_with_empty_history_does_nothing() {
    let mut app = make_app();
    // No history entries
    arrow(&mut app, KeyCode::Up);
    assert!(app.input.is_empty(), "Up with no history should leave input empty");
    assert_eq!(app.scroll_offset, 0);
}

// --- Ctrl+P during Pending state discards pending ---

#[test]
fn test_ctrl_p_during_pending_discards_deferred() {
    let mut app = make_app();
    app.input_history.push("first".into());
    app.input_history.push("second".into());
    // Up → Pending (deferred)
    let action = handle_key(&mut app, key(KeyCode::Up));
    assert!(matches!(action, InputAction::StartArrowDebounce));
    assert!(app.input.is_empty(), "Up is deferred, input still empty");
    // Ctrl+P → navigates history; discard pending debounce
    handle_key(&mut app, ctrl('p'));
    assert_eq!(app.input, "second", "Ctrl+P navigates history");
    // Stale timer fires — discarded, no second navigation
    resolve_arrow_debounce(&mut app);
    assert_eq!(app.input, "second", "stale timer is no-op after discard");
}

// --- Burst then different action ---

#[test]
fn test_scroll_burst_then_type_resets_debounce() {
    let mut app = make_app();
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Up));
    assert!(app.scroll_offset > 0, "burst should scroll");
    // Typing resets scroll and clears Scrolling state
    handle_key(&mut app, key(KeyCode::Char('x')));
    assert_eq!(app.scroll_offset, 0, "typing resets scroll");
    assert_eq!(app.input, "x");
}

// --- Sequential burst: up then down ---

#[test]
fn test_sequential_up_then_down_bursts() {
    let mut app = make_app();
    // Up burst
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Up));
    handle_key(&mut app, key(KeyCode::Up));
    let after_up = app.scroll_offset;
    assert!(after_up > 0);
    // Wait for Scrolling to expire
    std::thread::sleep(std::time::Duration::from_millis(200));
    // Down burst
    handle_key(&mut app, key(KeyCode::Down));
    handle_key(&mut app, key(KeyCode::Down));
    handle_key(&mut app, key(KeyCode::Down));
    assert!(app.scroll_offset < after_up, "Down burst should reduce offset");
}
