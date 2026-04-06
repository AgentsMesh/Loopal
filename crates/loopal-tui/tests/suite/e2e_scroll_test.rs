/// E2E integration tests for scroll behavior through the full TUI event loop.
///
/// Mouse scroll uses `AppEvent::ScrollUp/ScrollDown` (deterministic, no heuristic).
/// Keyboard Up/Down always navigates history via `handle_key_action`.
///
/// Regression tests for:
/// - Bug 1: Mouse scroll events scroll content, not history
/// - Bug 2: Single keyboard arrow navigates history, not scroll
/// - Bug 3: Scroll offset is compensated when content grows during streaming
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::{mpsc, watch};

use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, InterruptSignal, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::event::{AppEvent, EventHandler};
use loopal_tui::run_tui_loop;

fn build_scroll_rig() -> (
    Terminal<TestBackend>,
    App,
    EventHandler,
    mpsc::Sender<AppEvent>,
) {
    let (ctrl_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (q_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "test-model".into(),
        "act".into(),
        ctrl_tx,
        perm_tx,
        q_tx,
        InterruptSignal::new(),
        Arc::new(watch::channel(0u64).0),
    );
    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).unwrap();
    let mut app = App::new(session, std::env::temp_dir());
    app.input_history.push("history-entry".into());
    let (tx, rx) = mpsc::channel::<AppEvent>(256);
    let events = EventHandler::from_channel(tx.clone(), rx);
    (terminal, app, events, tx)
}

fn up_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)
}

fn page_up_key() -> KeyEvent {
    KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE)
}

fn ctrl_d() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)
}

fn stream_event(text: &str) -> AppEvent {
    AppEvent::Agent(AgentEvent::root(AgentEventPayload::Stream {
        text: text.to_string(),
    }))
}

// --- Bug 1 regression: scroll events → scroll content, not history ---

#[tokio::test]
async fn test_scroll_events_scroll_not_history() {
    let (mut terminal, mut app, events, tx) = build_scroll_rig();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        // Mouse wheel fires ScrollUp events (deterministic, no batch detection).
        let _ = tx.send(AppEvent::ScrollUp).await;
        let _ = tx.send(AppEvent::ScrollUp).await;
        let _ = tx.send(AppEvent::ScrollUp).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        let _ = tx.send(AppEvent::Key(ctrl_d())).await;
    });

    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app),
    )
    .await;

    assert!(
        app.content_scroll.offset > 0,
        "ScrollUp should scroll content, got offset={}",
        app.content_scroll.offset
    );
    assert!(
        app.input.is_empty(),
        "ScrollUp should NOT navigate history, got input={:?}",
        app.input
    );
}

#[tokio::test]
async fn test_scroll_down_decreases_offset() {
    let (mut terminal, mut app, events, tx) = build_scroll_rig();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        // Scroll up first to create an offset
        let _ = tx.send(AppEvent::ScrollUp).await;
        let _ = tx.send(AppEvent::ScrollUp).await;
        let _ = tx.send(AppEvent::ScrollUp).await;
        let _ = tx.send(AppEvent::ScrollUp).await;
        // Then scroll down
        let _ = tx.send(AppEvent::ScrollDown).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        let _ = tx.send(AppEvent::Key(ctrl_d())).await;
    });

    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app),
    )
    .await;

    assert_eq!(
        app.content_scroll.offset, 9,
        "4 ScrollUp (12) - 1 ScrollDown (3) = 9"
    );
}

// --- Bug 2 regression: single arrow → history, not scroll ---

#[tokio::test]
async fn test_single_arrow_navigates_history() {
    let (mut terminal, mut app, events, tx) = build_scroll_rig();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        // Single Up event = keyboard press → history navigation
        let _ = tx.send(AppEvent::Key(up_key())).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        let _ = tx.send(AppEvent::Key(ctrl_d())).await;
    });

    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app),
    )
    .await;

    assert_eq!(
        app.input, "history-entry",
        "single Up should navigate history"
    );
    assert_eq!(
        app.content_scroll.offset, 0,
        "single Up should not scroll content"
    );
}

// --- Bug 3 regression: scroll offset compensated during streaming ---

#[tokio::test]
async fn test_scroll_offset_stable_during_streaming() {
    let (mut terminal, mut app, events, tx) = build_scroll_rig();

    let tx2 = tx.clone();
    let tx3 = tx.clone();
    tokio::spawn(async move {
        // 1. Send initial content (enough to make the area scrollable)
        let initial: String = (0..40).map(|i| format!("line {i}\n")).collect();
        let _ = tx.send(stream_event(&initial)).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 2. Scroll up via PageUp
        let _ = tx2.send(AppEvent::Key(page_up_key())).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 3. More content arrives while scrolled up
        let more: String = (0..60).map(|i| format!("line {i}\n")).collect();
        let _ = tx3.send(stream_event(&more)).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        // 4. Quit
        let _ = tx3.send(AppEvent::Key(ctrl_d())).await;
    });

    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app),
    )
    .await;

    // PageUp sets offset=10. Compensation should increase it further
    // because streaming content grew while we were pinned.
    assert!(
        app.content_scroll.offset > 10,
        "offset should be compensated for growth: got {}",
        app.content_scroll.offset
    );
}
