/// Unit tests for ContentScroll growth compensation logic.
///
/// Verifies that the viewport stays anchored when content grows while scrolled up,
/// and that auto-follow (offset=0) is not affected by content growth.
use std::sync::Arc;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::{mpsc, watch};

use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, InterruptSignal, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::views::progress::ContentScroll;

fn make_app() -> App {
    let (ctrl_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (q_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        ctrl_tx,
        perm_tx,
        q_tx,
        InterruptSignal::new(),
        Arc::new(watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn stream_event(text: &str) -> AgentEvent {
    AgentEvent::root(AgentEventPayload::Stream {
        text: text.to_string(),
    })
}

fn render_once(terminal: &mut Terminal<TestBackend>, scroll: &mut ContentScroll, app: &App) {
    let conv = app.snapshot_active_conversation();
    terminal
        .draw(|f| scroll.render(f, &conv, f.area()))
        .unwrap();
}

#[test]
fn test_no_compensation_at_bottom() {
    let mut app = make_app();
    let mut scroll = ContentScroll::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    app.dispatch_event(stream_event("initial content\n"));
    render_once(&mut terminal, &mut scroll, &app);
    assert_eq!(scroll.offset, 0);

    app.dispatch_event(stream_event("initial content\nmore lines\nand more\n"));
    render_once(&mut terminal, &mut scroll, &app);
    assert_eq!(scroll.offset, 0, "offset should stay 0 when at bottom");
}

#[test]
fn test_compensation_when_pinned() {
    let mut app = make_app();
    let mut scroll = ContentScroll::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    let long_text: String = (0..40).map(|i| format!("line {i}\n")).collect();
    app.dispatch_event(stream_event(&long_text));
    render_once(&mut terminal, &mut scroll, &app);

    scroll.scroll_up(10);
    assert_eq!(scroll.offset, 10);

    render_once(&mut terminal, &mut scroll, &app);
    let offset_before = scroll.offset;

    let more: String = (40..50).map(|i| format!("line {i}\n")).collect();
    let combined = format!("{long_text}{more}");
    app.dispatch_event(stream_event(&combined));
    render_once(&mut terminal, &mut scroll, &app);

    assert!(
        scroll.offset > offset_before,
        "offset should increase to compensate: before={offset_before}, after={}",
        scroll.offset
    );
}

#[test]
fn test_no_blowup_first_render_with_offset() {
    let mut app = make_app();
    let mut scroll = ContentScroll::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    scroll.scroll_up(5);

    let text: String = (0..30).map(|i| format!("line {i}\n")).collect();
    app.dispatch_event(stream_event(&text));
    render_once(&mut terminal, &mut scroll, &app);

    assert!(
        scroll.offset < 100,
        "first render should not cause massive offset: got {}",
        scroll.offset
    );
}

#[test]
fn test_reset_clears_prev_total() {
    let mut app = make_app();
    let mut scroll = ContentScroll::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    let text: String = (0..30).map(|i| format!("line {i}\n")).collect();
    app.dispatch_event(stream_event(&text));
    render_once(&mut terminal, &mut scroll, &app);
    scroll.scroll_up(10);
    render_once(&mut terminal, &mut scroll, &app);

    scroll.reset();
    assert_eq!(scroll.offset, 0);

    render_once(&mut terminal, &mut scroll, &app);
    assert_eq!(scroll.offset, 0, "after reset, should be at bottom");
}
