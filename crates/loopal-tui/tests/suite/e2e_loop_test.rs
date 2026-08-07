//! TUI loop E2E tests — verify `run_tui_loop` with TestBackend and injected events.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::{Backend, ClearType, TestBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use tokio::sync::{mpsc, watch};

use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, InterruptSignal, UserQuestionResponse,
};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::event::{AppEvent, EventHandler};
use loopal_tui::view_client::ViewClient;
use loopal_tui::{run_tui_loop, run_tui_loop_with_animation_clock};
use loopal_view_state::{ViewSnapshot, ViewStateReducer};

struct FrameRecordingBackend {
    inner: TestBackend,
    frame_tx: mpsc::UnboundedSender<String>,
}

impl FrameRecordingBackend {
    fn new(width: u16, height: u16, frame_tx: mpsc::UnboundedSender<String>) -> Self {
        Self {
            inner: TestBackend::new(width, height),
            frame_tx,
        }
    }
}

impl Backend for FrameRecordingBackend {
    type Error = Infallible;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)?;
        let buffer = self.inner.buffer();
        let mut text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        let _ = self.frame_tx.send(text);
        Ok(())
    }

    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> Result<Position, Self::Error> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> Result<(), Self::Error> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> Result<(), Self::Error> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> Result<Size, Self::Error> {
        self.inner.size()
    }

    fn window_size(&mut self) -> Result<WindowSize, Self::Error> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }
}

fn build_loop_rig() -> (
    Terminal<TestBackend>,
    App,
    EventHandler,
    mpsc::Sender<AppEvent>,
) {
    let (_agent_tx, _agent_rx) = mpsc::channel::<AgentEvent>(256);
    let (ctrl_tx, _ctrl_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _perm_rx) = mpsc::channel::<bool>(16);
    let (q_tx, _q_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let interrupt = InterruptSignal::new();
    let interrupt_tx = Arc::new(watch::channel(0u64).0);

    let session_ctrl = SessionController::new(ctrl_tx, perm_tx, q_tx, interrupt, interrupt_tx);

    let backend = TestBackend::new(80, 24);
    let terminal = Terminal::new(backend).unwrap();
    let app = App::new(session_ctrl, std::env::temp_dir());

    let (tx, rx) = mpsc::channel::<AppEvent>(256);
    let events = EventHandler::from_channel(tx.clone(), rx);

    (terminal, app, events, tx)
}

fn install_running_wire_snapshot(app: &mut App) {
    let mut hub_reducer = ViewStateReducer::new("main");
    hub_reducer.apply(AgentEventPayload::Running);
    hub_reducer.apply(AgentEventPayload::ToolCall {
        id: "tool-1".into(),
        name: "Read".into(),
        input: serde_json::json!({"file_path": "/tmp/input"}),
    });

    let wire = serde_json::to_string(&hub_reducer.snapshot()).expect("serialize snapshot");
    let snapshot: ViewSnapshot = serde_json::from_str(&wire).expect("deserialize snapshot");
    app.view_clients
        .insert("main".into(), ViewClient::from_snapshot("main", snapshot));

    assert_eq!(
        app.view_clients["main"]
            .state()
            .conversation()
            .turn_elapsed(),
        Duration::ZERO,
        "wire snapshot must reproduce the missing process-local timer"
    );
}

#[tokio::test(start_paused = true)]
async fn ticks_redraw_snapshot_spinners_without_new_agent_events() {
    let (_agent_tx, agent_rx) = mpsc::channel::<AgentEvent>(16);
    let (_resync_tx, resync_rx) = mpsc::channel::<()>(8);
    let events = EventHandler::new(agent_rx, resync_rx);
    let quit_tx = events.sender();

    let (ctrl_tx, _ctrl_rx) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _perm_rx) = mpsc::channel::<bool>(16);
    let (question_tx, _question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        ctrl_tx,
        perm_tx,
        question_tx,
        InterruptSignal::new(),
        Arc::new(watch::channel(0u64).0),
    );
    let mut app = App::new(session, std::env::temp_dir());
    install_running_wire_snapshot(&mut app);

    let (frame_tx, mut frame_rx) = mpsc::unbounded_channel();
    let mut terminal = Terminal::new(FrameRecordingBackend::new(80, 24, frame_tx)).unwrap();
    let mut clock = VecDeque::from([
        Duration::ZERO,
        Duration::from_millis(100),
        Duration::from_millis(200),
    ]);

    let loop_task = tokio::spawn(async move {
        run_tui_loop_with_animation_clock(&mut terminal, events, &mut app, || {
            clock.pop_front().expect("unexpected extra redraw")
        })
        .await
    });

    let initial = frame_rx.recv().await.expect("initial frame");
    let first_tick = frame_rx.recv().await.expect("first tick redraw");
    tokio::time::advance(Duration::from_millis(100)).await;
    let second_tick = frame_rx.recv().await.expect("second tick redraw");

    assert!(
        initial.contains("⠋ Working") && initial.contains("⠋ Read"),
        "initial snapshot frame did not use the local animation clock: {initial}"
    );
    assert!(
        first_tick.contains("⠙ Working") && first_tick.contains("⠙ Read"),
        "AppEvent::Tick did not redraw the next spinner frame: {first_tick}"
    );
    assert!(
        second_tick.contains("⠹ Working") && second_tick.contains("⠹ Read"),
        "the next AppEvent::Tick did not keep animation moving: {second_tick}"
    );

    quit_tx
        .send(AppEvent::Key(KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL,
        )))
        .await
        .expect("send quit");
    loop_task
        .await
        .expect("join TUI loop")
        .expect("run TUI loop");
}

#[tokio::test]
async fn test_e2e_loop_quit_on_ctrl_d() {
    let (mut terminal, mut app, events, tx) = build_loop_rig();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let _ = tx.send(AppEvent::Key(key)).await;
    });

    let result = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app),
    )
    .await;

    assert!(result.is_ok(), "loop should exit before timeout");
    assert!(app.exiting, "app.exiting should be true after Ctrl+D");
}

#[tokio::test]
async fn test_e2e_loop_renders_agent_event() {
    let (mut terminal, mut app, events, tx) = build_loop_rig();

    let tx2 = tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        let stream = AgentEvent::root(AgentEventPayload::Stream {
            text: "Agent says hi".into(),
        });
        let _ = tx.send(AppEvent::Agent(Box::new(stream))).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let _ = tx2.send(AppEvent::Key(key)).await;
    });

    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app),
    )
    .await;

    let conv = app.snapshot_active_conversation();
    assert!(
        conv.streaming_text.contains("Agent says hi"),
        "expected streaming_text to contain 'Agent says hi', got: {:?}",
        conv.streaming_text
    );
}

#[tokio::test]
async fn test_e2e_loop_ctrl_d_quits() {
    let (mut terminal, mut app, events, tx) = build_loop_rig();

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let _ = tx.send(AppEvent::Key(key)).await;
    });

    let result = tokio::time::timeout(
        Duration::from_secs(3),
        run_tui_loop(&mut terminal, events, &mut app),
    )
    .await;

    assert!(result.is_ok(), "loop should exit on Ctrl+D");
    assert!(app.exiting, "app.exiting should be true after Ctrl+D");
}
