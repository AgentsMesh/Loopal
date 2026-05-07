//! Shared helpers for `/goal` end-to-end tests.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_protocol::{AgentEvent, AgentEventPayload, ThreadGoalStatus};
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::goal::GoalRuntimeSession;
use loopal_storage::GoalStore;
use loopal_test_support::HarnessBuilder;
use loopal_tui::app::App;
use loopal_tui::command::CommandEffect;
use loopal_tui::dispatch_ops::handle_effect;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::TempDir;
use tokio::sync::mpsc;

use super::e2e_harness::TuiTestHarness;

pub(super) struct ProxyEmitter {
    pub tx: mpsc::Sender<AgentEvent>,
}

#[async_trait]
impl EventEmitter for ProxyEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> loopal_error::Result<()> {
        let _ = self.tx.send(AgentEvent::root(payload)).await;
        Ok(())
    }
}

pub(super) struct GoalScenario {
    pub _store_dir: TempDir,
    pub session: Arc<GoalRuntimeSession>,
    pub proxy_rx: mpsc::Receiver<AgentEvent>,
    pub harness: TuiTestHarness,
}

pub(super) async fn setup() -> GoalScenario {
    let store_dir = TempDir::new().unwrap();
    let store = Arc::new(GoalStore::with_base_dir(store_dir.path().to_path_buf()));
    let (proxy_tx, proxy_rx) = mpsc::channel::<AgentEvent>(64);
    let session = Arc::new(GoalRuntimeSession::new(
        "e2e-goal-session".into(),
        store,
        Box::new(ProxyEmitter { tx: proxy_tx }),
    ));

    let inner = HarnessBuilder::new()
        .messages(vec![])
        .goal_session(session.clone())
        .build_spawned()
        .await;

    let terminal = Terminal::new(TestBackend::new(120, 24)).unwrap();
    let app = App::new(
        inner.session_ctrl.clone(),
        inner.fixture.path().to_path_buf(),
    );
    let harness = TuiTestHarness {
        terminal,
        app,
        inner,
    };

    GoalScenario {
        _store_dir: store_dir,
        session,
        proxy_rx,
        harness,
    }
}

pub(super) fn drain_proxy(scenario: &mut GoalScenario) {
    while let Ok(event) = scenario.proxy_rx.try_recv() {
        scenario.harness.app.dispatch_event(event);
    }
}

pub(super) async fn run_goal(scenario: &mut GoalScenario, arg: Option<&str>) -> CommandEffect {
    let handler = scenario
        .harness
        .app
        .command_registry
        .find("/goal")
        .expect("/goal handler must be registered");
    let effect = handler.execute(&mut scenario.harness.app, arg).await;
    handle_effect(&mut scenario.harness.app, copy_effect(&effect)).await;
    effect
}

fn copy_effect(effect: &CommandEffect) -> CommandEffect {
    match effect {
        CommandEffect::Reply(s) => CommandEffect::Reply(s.clone()),
        CommandEffect::Done => CommandEffect::Done,
        _ => panic!(
            "unexpected effect for /goal: {:?}",
            std::mem::discriminant(effect)
        ),
    }
}

pub(super) async fn wait_for_status(
    scenario: &mut GoalScenario,
    expected: ThreadGoalStatus,
    timeout: Duration,
) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        drain_proxy(scenario);
        if let Some(snap) = scenario.session.snapshot().await.unwrap()
            && snap.status == expected
        {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            let snap = scenario.session.snapshot().await.unwrap();
            panic!("timed out waiting for {expected:?}; current = {snap:?}");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

pub(super) fn last_system_message(app: &App) -> String {
    let conv = app.snapshot_active_conversation();
    conv.messages
        .iter()
        .rev()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone())
        .unwrap_or_default()
}
