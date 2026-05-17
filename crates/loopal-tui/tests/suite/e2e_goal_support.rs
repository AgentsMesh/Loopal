//! Shared helpers for `/goal` end-to-end tests.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_protocol::{AgentEvent, AgentEventPayload, ThreadGoalStatus};
use loopal_provider_api::Provider;
use loopal_runtime::frontend::traits::EventEmitter;
use loopal_runtime::goal::GoalRuntimeSession;
use loopal_storage::GoalStore;
use loopal_test_support::{HarnessBuilder, mock_provider::HangingProvider};
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
    pub status_history: Vec<ThreadGoalStatus>,
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

    // reason: GoalCreate now triggers a kickoff continuation turn. Stub the
    // LLM with a never-completing stream so the runner enters Running but
    // does not progress — goal status stays stable in Active for status-bar
    // assertions, and tests that need other states drive `session` directly
    // (the control pipeline is covered separately by the create test).
    let inner = HarnessBuilder::new()
        .messages(vec![])
        .goal_session(session.clone())
        .kernel_setup(|k| {
            k.register_provider(Arc::new(HangingProvider) as Arc<dyn Provider>);
        })
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
        status_history: Vec::new(),
    }
}

pub(super) fn drain_proxy(scenario: &mut GoalScenario) {
    while let Ok(event) = scenario.proxy_rx.try_recv() {
        if let AgentEventPayload::ThreadGoalUpdated {
            goal: Some(ref g), ..
        } = event.payload
        {
            scenario.status_history.push(g.status);
        }
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
        if scenario.status_history.contains(&expected) {
            return;
        }
        if let Some(snap) = scenario.session.snapshot().await.unwrap()
            && snap.status == expected
        {
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            let snap = scenario.session.snapshot().await.unwrap();
            panic!(
                "timed out waiting for {expected:?}; current = {snap:?}; history = {:?}",
                scenario.status_history
            );
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

pub(super) async fn drain_until_running(scenario: &mut GoalScenario, timeout: Duration) {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        while let Ok(ev) = scenario.harness.inner.event_rx.try_recv() {
            if matches!(
                ev,
                AgentEvent {
                    payload: AgentEventPayload::Running,
                    ..
                }
            ) {
                return;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for AgentEventPayload::Running");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
