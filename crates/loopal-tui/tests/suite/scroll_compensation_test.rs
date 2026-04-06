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
use loopal_tui::views::progress::ContentScroll;

fn make_session() -> SessionController {
    let (ctrl_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (q_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    SessionController::new(
        "test".into(),
        "act".into(),
        ctrl_tx,
        perm_tx,
        q_tx,
        InterruptSignal::new(),
        Arc::new(watch::channel(0u64).0),
    )
}

fn stream_event(text: &str) -> AgentEvent {
    AgentEvent::root(AgentEventPayload::Stream {
        text: text.to_string(),
    })
}

fn render_once(
    terminal: &mut Terminal<TestBackend>,
    scroll: &mut ContentScroll,
    session: &SessionController,
) {
    let state = session.lock();
    terminal
        .draw(|f| scroll.render(f, &state, f.area()))
        .unwrap();
}

// --- Auto-follow: offset=0 stays at 0 ---

#[test]
fn test_no_compensation_at_bottom() {
    let session = make_session();
    let mut scroll = ContentScroll::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    // Establish prev_total with some content
    session.handle_event(stream_event("initial content\n"));
    render_once(&mut terminal, &mut scroll, &session);
    assert_eq!(scroll.offset, 0);

    // Add more streaming content
    session.handle_event(stream_event("initial content\nmore lines\nand more\n"));
    render_once(&mut terminal, &mut scroll, &session);
    assert_eq!(scroll.offset, 0, "offset should stay 0 when at bottom");
}

// --- Pinned: offset grows with content ---

#[test]
fn test_compensation_when_pinned() {
    let session = make_session();
    let mut scroll = ContentScroll::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    // Generate enough content to be scrollable
    let long_text: String = (0..40).map(|i| format!("line {i}\n")).collect();
    session.handle_event(stream_event(&long_text));
    render_once(&mut terminal, &mut scroll, &session);

    // Scroll up
    scroll.scroll_up(10);
    assert_eq!(scroll.offset, 10);

    // Render with same content to establish prev_total at this offset
    render_once(&mut terminal, &mut scroll, &session);
    let offset_before = scroll.offset;

    // Add more streaming content
    let more: String = (40..50).map(|i| format!("line {i}\n")).collect();
    let combined = format!("{long_text}{more}");
    session.handle_event(stream_event(&combined));
    render_once(&mut terminal, &mut scroll, &session);

    assert!(
        scroll.offset > offset_before,
        "offset should increase to compensate: before={offset_before}, after={}",
        scroll.offset
    );
}

// --- First frame: no blowup when prev_total=0 ---

#[test]
fn test_no_blowup_first_render_with_offset() {
    let session = make_session();
    let mut scroll = ContentScroll::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    // Pre-set offset before any render (simulates scroll arriving before first paint)
    scroll.scroll_up(5);

    let text: String = (0..30).map(|i| format!("line {i}\n")).collect();
    session.handle_event(stream_event(&text));
    render_once(&mut terminal, &mut scroll, &session);

    // Should NOT blow up: offset should remain small, not jump by total_lines
    assert!(
        scroll.offset < 100,
        "first render should not cause massive offset: got {}",
        scroll.offset
    );
}

// --- Reset clears compensation state ---

#[test]
fn test_reset_clears_prev_total() {
    let session = make_session();
    let mut scroll = ContentScroll::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

    // Establish state with content
    let text: String = (0..30).map(|i| format!("line {i}\n")).collect();
    session.handle_event(stream_event(&text));
    render_once(&mut terminal, &mut scroll, &session);
    scroll.scroll_up(10);
    render_once(&mut terminal, &mut scroll, &session);

    // Reset (simulates view switch)
    scroll.reset();
    assert_eq!(scroll.offset, 0);

    // Render with content again — should not falsely compensate
    render_once(&mut terminal, &mut scroll, &session);
    assert_eq!(scroll.offset, 0, "after reset, should be at bottom");
}
