use std::sync::Arc;

use async_trait::async_trait;
use loopal_error::{LoopalError, Result};
use loopal_kernel::Kernel;
use loopal_protocol::{AgentEvent, AgentEventPayload, PermissionIntentRequest};
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::frontend::{AgentFrontend, EventEmitter};
use loopal_runtime::{AgentConfig, AgentDeps, AgentLoopParamsBuilder, InterruptHandle};
use loopal_test_support::TestFixture;
use loopal_tool_api::PermissionDecision;
use tokio::sync::{Mutex, mpsc};

use super::make_test_budget;

struct TrackedFrontend {
    events: mpsc::Sender<AgentEvent>,
    inputs: Mutex<mpsc::Receiver<AgentInput>>,
}

#[derive(Clone)]
struct TrackedEmitter(mpsc::Sender<AgentEvent>);

async fn emit_event(events: &mpsc::Sender<AgentEvent>, payload: AgentEventPayload) -> Result<()> {
    events
        .send(AgentEvent::root(payload))
        .await
        .map_err(|error| LoopalError::Other(format!("event channel closed: {error}")))
}

#[async_trait]
impl EventEmitter for TrackedEmitter {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        emit_event(&self.0, payload).await
    }
}

#[async_trait]
impl AgentFrontend for TrackedFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        emit_event(&self.events, payload).await
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        self.inputs.lock().await.recv().await
    }

    async fn try_recv_input(&self) -> std::result::Result<AgentInput, mpsc::error::TryRecvError> {
        self.inputs.lock().await.try_recv()
    }

    async fn drain_pending(&self) -> Vec<AgentInput> {
        let mut inputs = self.inputs.lock().await;
        let mut pending = Vec::new();
        while let Ok(input) = inputs.try_recv() {
            pending.push(input);
        }
        pending
    }

    async fn request_permission(&self, _request: &PermissionIntentRequest) -> PermissionDecision {
        PermissionDecision::Deny
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(TrackedEmitter(self.events.clone()))
    }
}

pub fn make_runner_with_tracked_kernel(
    kernel: Arc<Kernel>,
) -> (
    loopal_runtime::agent_loop::AgentLoopRunner,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<AgentInput>,
) {
    let (mut runner, event_rx, input_tx) = make_idle_runner_with_tracked_kernel(kernel);
    runner.start_turn_record(loopal_turn::TurnTrigger::Resume);
    (runner, event_rx, input_tx)
}

pub fn make_idle_runner_with_tracked_kernel(
    kernel: Arc<Kernel>,
) -> (
    loopal_runtime::agent_loop::AgentLoopRunner,
    mpsc::Receiver<AgentEvent>,
    mpsc::Sender<AgentInput>,
) {
    let fixture = TestFixture::new();
    let (event_tx, event_rx) = mpsc::channel(64);
    let (input_tx, input_rx) = mpsc::channel(16);
    let frontend = Arc::new(TrackedFrontend {
        events: event_tx,
        inputs: Mutex::new(input_rx),
    });
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        AgentDeps {
            kernel,
            frontend,
            session_manager: fixture.session_manager(),
            decision_context: loopal_runtime::frontend::DecisionContext::with_cwd("/tmp/test"),
            protected_effect_audit: super::noop_protected_effect_audit(),
        },
        fixture.test_session("test-tracked-kernel"),
        make_test_budget(),
        InterruptHandle::new(),
    )
    .build();
    let runner = loopal_runtime::agent_loop::AgentLoopRunner::new(params);
    (runner, event_rx, input_tx)
}
