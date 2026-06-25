use std::sync::Arc;

use loopal_protocol::{
    AgentEvent, AgentEventPayload, CompactPhase, CompactionSummary, ControlCommand,
    InterruptSignal, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::render::draw;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::{mpsc, watch};

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (q_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        perm_tx,
        q_tx,
        InterruptSignal::new(),
        Arc::new(watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn progress_event(phase: CompactPhase, detail: Option<&str>) -> AgentEvent {
    AgentEvent::root(AgentEventPayload::CompactProgress {
        phase,
        detail: detail.map(String::from),
    })
}

fn render_frame(app: &mut App) -> String {
    let mut term = Terminal::new(TestBackend::new(120, 24)).unwrap();
    term.draw(|f| draw(f, app)).unwrap();
    let buf = term.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            text.push_str(buf.cell((x, y)).map_or(" ", |c| c.symbol()));
        }
        text.push('\n');
    }
    text
}

#[test]
fn compact_progress_microcompact_renders_in_frame() {
    let mut app = make_app();
    app.dispatch_event(progress_event(CompactPhase::Microcompact, None));
    let frame = render_frame(&mut app);
    assert!(
        frame.contains("microcompact"),
        "frame must show Microcompact phase label, got:\n{frame}",
    );
}

#[test]
fn compact_progress_summarize_renders_in_frame() {
    let mut app = make_app();
    app.dispatch_event(progress_event(CompactPhase::Summarize, None));
    let frame = render_frame(&mut app);
    assert!(
        frame.contains("summariz"),
        "frame must show Summarize phase label, got:\n{frame}",
    );
}

#[test]
fn compact_progress_rehydrate_with_detail_renders_both() {
    let mut app = make_app();
    app.dispatch_event(progress_event(
        CompactPhase::Rehydrate,
        Some("5 files, 12.4K"),
    ));
    let frame = render_frame(&mut app);
    assert!(
        frame.contains("rehydrat"),
        "frame must show Rehydrate label, got:\n{frame}",
    );
    assert!(
        frame.contains("5 files, 12.4K"),
        "frame must show detail string, got:\n{frame}",
    );
}

#[test]
fn compact_progress_done_clears_banner_from_frame() {
    let mut app = make_app();
    app.dispatch_event(progress_event(CompactPhase::Summarize, None));
    assert!(render_frame(&mut app).contains("summariz"));
    app.dispatch_event(progress_event(CompactPhase::Done, None));
    let after = render_frame(&mut app);
    assert!(
        !after.contains("summariz"),
        "Done phase must remove banner text from frame, got:\n{after}",
    );
}

#[test]
fn compacted_event_clears_stale_banner_from_frame() {
    let mut app = make_app();
    app.dispatch_event(progress_event(CompactPhase::Rehydrate, None));
    assert!(render_frame(&mut app).contains("rehydrat"));
    app.dispatch_event(AgentEvent::root(AgentEventPayload::Compacted(
        CompactionSummary {
            kept: 4,
            summarized: 80,
            tokens_before: 40_000,
            tokens_after: 4_000,
            strategy: "auto".into(),
            summary_msg_id: None,
            files_rehydrated: 5,
        },
    )));
    let after = render_frame(&mut app);
    assert!(
        !after.contains("rehydrat"),
        "Compacted event must clear stale banner from frame, got:\n{after}",
    );
}

#[test]
fn phase_transition_replaces_text_in_frame() {
    let mut app = make_app();
    app.dispatch_event(progress_event(CompactPhase::Microcompact, None));
    assert!(render_frame(&mut app).contains("microcompact"));

    app.dispatch_event(progress_event(CompactPhase::Summarize, None));
    let frame = render_frame(&mut app);
    assert!(
        !frame.contains("microcompact"),
        "previous Microcompact text must be replaced",
    );
    assert!(
        frame.contains("summariz"),
        "Summarize text must appear after transition",
    );
}

#[test]
fn awaiting_input_after_progress_clears_banner_from_frame() {
    let mut app = make_app();
    app.dispatch_event(progress_event(CompactPhase::Summarize, None));
    assert!(render_frame(&mut app).contains("summariz"));
    app.dispatch_event(AgentEvent::root(AgentEventPayload::AwaitingInput));
    let after = render_frame(&mut app);
    assert!(
        !after.contains("summariz"),
        "AwaitingInput must clear stuck banner from frame, got:\n{after}",
    );
}

#[test]
fn no_compact_event_means_no_banner_in_frame() {
    let mut app = make_app();
    let frame = render_frame(&mut app);
    assert!(
        !frame.contains("microcompact")
            && !frame.contains("summariz")
            && !frame.contains("rehydrat"),
        "fresh App must render no banner text, got:\n{frame}",
    );
}
