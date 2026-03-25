//! TUI loop E2E tests — verify `run_tui_loop` with TestBackend and injected events.

use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::{mpsc, watch};

use loopal_agent::router::MessageRouter;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, InterruptSignal, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::command::CommandEntry;
use loopal_tui::event::{AppEvent, EventHandler};
use loopal_tui::run_tui_loop;

/// Build a minimal TUI loop test rig: App + Terminal<TestBackend> + EventHandler.
fn build_loop_rig() -> (
    Terminal<TestBackend>,
    App,
    EventHandler,
    mpsc::Sender<AppEvent>,
    Arc<MessageRouter>,
    mpsc::Receiver<AgentEvent>,
) {
    let (agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(256);
    let (ctrl_tx, _ctrl_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _perm_rx) = mpsc::channel::<bool>(16);
    let (q_tx, _q_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let interrupt = InterruptSignal::new();
    let interrupt_tx = Arc::new(watch::channel(0u64).0);

    let session_ctrl = SessionController::new(
        "test-model".into(),
        "act".into(),
        ctrl_tx,
        perm_tx,
        q_tx,
        interrupt,
        interrupt_tx,
    );

    let router = Arc::new(MessageRouter::new(agent_tx));
    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).unwrap();
    let app = App::new(
        session_ctrl,
        Vec::<CommandEntry>::new(),
        std::env::temp_dir(),
    );

    let (tx, rx) = mpsc::channel::<AppEvent>(256);
    let events = EventHandler::from_channel(tx.clone(), rx);

    (terminal, app, events, tx, router, agent_rx)
}

#[tokio::test]
async fn test_e2e_loop_quit_on_ctrl_d() {
    let (mut terminal, mut app, events, tx, router, _agent_rx) = build_loop_rig();

    // Inject Ctrl+D after a short delay (Ctrl+C now clears input, Ctrl+D quits)
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let _ = tx.send(AppEvent::Key(key)).await;
    });

    let result = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app, &router, "main"),
    )
    .await;

    assert!(result.is_ok(), "loop should exit before timeout");
    assert!(app.exiting, "app.exiting should be true after Ctrl+D");
}

#[tokio::test]
async fn test_e2e_loop_renders_agent_event() {
    let (mut terminal, mut app, events, tx, router, _agent_rx) = build_loop_rig();

    // Inject a stream event then quit
    let tx2 = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        let stream = AgentEvent::root(AgentEventPayload::Stream {
            text: "Agent says hi".into(),
        });
        let _ = tx.send(AppEvent::Agent(stream)).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let _ = tx2.send(AppEvent::Key(key)).await;
    });

    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app, &router, "main"),
    )
    .await;

    // Verify the streaming text was captured in session state
    let state = app.session.lock();
    assert!(
        state.streaming_text.contains("Agent says hi"),
        "expected streaming_text to contain 'Agent says hi', got: {:?}",
        state.streaming_text
    );
}

#[tokio::test]
async fn test_e2e_loop_ctrl_d_quits() {
    let (mut terminal, mut app, events, tx, router, _agent_rx) = build_loop_rig();

    // Inject Ctrl+D — should trigger Quit action
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let _ = tx.send(AppEvent::Key(key)).await;
    });

    let result = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app, &router, "main"),
    )
    .await;

    assert!(result.is_ok(), "loop should exit on Ctrl+D");
    assert!(app.exiting, "app.exiting should be true after Ctrl+D");
}
