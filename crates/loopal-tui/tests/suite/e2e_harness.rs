//! TUI integration test harness — wires HarnessBuilder → agent_loop → SessionController → render.

use loopal_error::LoopalError;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::StreamChunk;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use loopal_test_support::events::{self, DEFAULT_TIMEOUT};
use loopal_test_support::{HarnessBuilder, SpawnedHarness};
use loopal_tui::app::App;
use loopal_tui::command::CommandEntry;
use loopal_tui::render::draw;

/// Full-stack TUI integration test harness.
///
/// `inner.session_ctrl` shares channels with the `UnifiedFrontend` inside the
/// agent loop — permission, control, and question flows are correctly wired.
pub struct TuiTestHarness {
    pub terminal: Terminal<TestBackend>,
    pub app: App,
    pub inner: SpawnedHarness,
}

/// Build a TUI integration harness with mock provider calls.
pub async fn build_tui_harness(
    calls: Vec<Vec<Result<StreamChunk, LoopalError>>>,
    width: u16,
    height: u16,
) -> TuiTestHarness {
    let inner = HarnessBuilder::new().calls(calls).build_spawned().await;

    let terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let app = App::new(
        inner.session_ctrl.clone(),
        Vec::<CommandEntry>::new(),
        inner.fixture.path().to_path_buf(),
    );

    TuiTestHarness {
        terminal,
        app,
        inner,
    }
}

impl TuiTestHarness {
    /// Collect agent events until idle, feeding each to SessionController.
    pub async fn collect_until_idle(&mut self) -> Vec<AgentEventPayload> {
        let session = &self.app.session;
        events::collect_until_idle(&mut self.inner.event_rx, DEFAULT_TIMEOUT, |event| {
            session.handle_event(event.clone());
        })
        .await
    }

    /// Render the current state and return the buffer as plain text.
    pub fn render_text(&mut self) -> String {
        self.terminal.draw(|f| draw(f, &mut self.app)).unwrap();
        let buf = self.terminal.backend().buffer().clone();
        let mut text = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                text.push_str(buf.cell((x, y)).map_or(" ", |c| c.symbol()));
            }
            text.push('\n');
        }
        text
    }
}
