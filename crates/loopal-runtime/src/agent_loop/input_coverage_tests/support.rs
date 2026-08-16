use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use loopal_context::ContextBudget;
use loopal_error::{LoopalError, Result};
use loopal_protocol::{
    AgentEventPayload, Envelope, MessageSource, PermissionIntent, PermissionIntentRequest,
    PermissionReceipt, WorkflowRunId, WorkflowRunState, WorkflowTerminalDeliveryId,
    WorkflowTerminalNotification, WorkflowTerminalOutcome,
};
use loopal_tool_api::PermissionDecision;
use tokio::sync::mpsc;

use crate::SessionManager;
use crate::agent_input::AgentInput;
use crate::agent_loop::{
    AgentConfig, AgentDeps, AgentLoopParamsBuilder, AgentLoopRunner, InterruptHandle,
};
use crate::frontend::{AgentFrontend, DecisionContext, EventEmitter, PermissionOutcome};

pub(super) const FAIL_AWAITING_INPUT: u8 = 1;
pub(super) const FAIL_ERROR: u8 = 2;
pub(super) const FAIL_FINISHED: u8 = 4;
pub(super) const FAIL_RUNNING: u8 = 8;
pub(super) const FAIL_GATE: u8 = 16;
pub(super) const FAIL_PERMISSION: u8 = 32;

#[derive(Clone, Copy)]
pub(super) enum PermissionBehavior {
    AllowWithoutReceipt,
    AllowWithValidReceipt,
}

pub(super) struct TestFrontend {
    inputs: tokio::sync::Mutex<mpsc::Receiver<AgentInput>>,
    drained: Mutex<Vec<AgentInput>>,
    events: Mutex<Vec<AgentEventPayload>>,
    fail_mask: AtomicU8,
    permission: Mutex<PermissionBehavior>,
}

impl TestFrontend {
    fn new(inputs: mpsc::Receiver<AgentInput>) -> Self {
        Self {
            inputs: tokio::sync::Mutex::new(inputs),
            drained: Mutex::new(Vec::new()),
            events: Mutex::new(Vec::new()),
            fail_mask: AtomicU8::new(0),
            permission: Mutex::new(PermissionBehavior::AllowWithoutReceipt),
        }
    }

    pub(super) fn set_drained(&self, inputs: Vec<AgentInput>) {
        *self.drained.lock().unwrap() = inputs;
    }

    pub(super) fn set_fail_mask(&self, mask: u8) {
        self.fail_mask.store(mask, Ordering::Release);
    }

    pub(super) fn set_permission(&self, behavior: PermissionBehavior) {
        *self.permission.lock().unwrap() = behavior;
    }
}

struct NullEmitter;

#[async_trait]
impl EventEmitter for NullEmitter {
    async fn emit(&self, _payload: AgentEventPayload) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl AgentFrontend for TestFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let failure = match payload {
            AgentEventPayload::AwaitingInput => FAIL_AWAITING_INPUT,
            AgentEventPayload::Error { .. } => FAIL_ERROR,
            AgentEventPayload::Finished => FAIL_FINISHED,
            AgentEventPayload::Running => FAIL_RUNNING,
            AgentEventPayload::ContinuationGateChanged(_) => FAIL_GATE,
            AgentEventPayload::PermissionModeChanged { .. } => FAIL_PERMISSION,
            _ => 0,
        };
        if self.fail_mask.load(Ordering::Acquire) & failure != 0 {
            return Err(LoopalError::Other("injected event failure".into()));
        }
        self.events.lock().unwrap().push(payload);
        Ok(())
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        self.inputs.lock().await.recv().await
    }

    async fn try_recv_input(&self) -> std::result::Result<AgentInput, mpsc::error::TryRecvError> {
        self.inputs.lock().await.try_recv()
    }

    async fn request_permission(&self, _request: &PermissionIntentRequest) -> PermissionDecision {
        PermissionDecision::Allow
    }

    async fn request_permission_outcome(
        &self,
        request: &PermissionIntentRequest,
    ) -> PermissionOutcome {
        let behavior = *self.permission.lock().unwrap();
        let receipt = match behavior {
            PermissionBehavior::AllowWithoutReceipt => None,
            PermissionBehavior::AllowWithValidReceipt => {
                let intent =
                    PermissionIntent::bind(request.intent_seed.clone(), 7, 11, "runtime-coverage")
                        .unwrap();
                Some(PermissionReceipt::issue_for_intent(&intent, "runtime-test").unwrap())
            }
        };
        PermissionOutcome {
            decision: PermissionDecision::Allow,
            reason: String::new(),
            duration_ms: 0,
            receipt,
        }
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(NullEmitter)
    }

    async fn drain_pending(&self) -> Vec<AgentInput> {
        std::mem::take(&mut *self.drained.lock().unwrap())
    }
}

pub(super) struct Fixture {
    pub(super) runner: AgentLoopRunner,
    pub(super) frontend: Arc<TestFrontend>,
    pub(super) input_tx: mpsc::Sender<AgentInput>,
    pub(super) temp: tempfile::TempDir,
}

pub(super) fn make_fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let session_manager = SessionManager::with_base_dir(temp.path().join("state"));
    let session_id = format!("runtime-coverage-{}", uuid::Uuid::new_v4().simple());
    let session = session_manager
        .create_session_with_id(temp.path(), "test-model", &session_id)
        .unwrap();
    let (input_tx, input_rx) = mpsc::channel(16);
    let frontend = Arc::new(TestFrontend::new(input_rx));
    let params = AgentLoopParamsBuilder::new(
        AgentConfig::default(),
        AgentDeps {
            kernel: Arc::new(loopal_kernel::Kernel::new(Default::default()).unwrap()),
            frontend: frontend.clone(),
            session_manager,
            decision_context: DecisionContext::with_cwd(temp.path().to_string_lossy()),
            protected_effect_audit: Arc::new(loopal_tool_api::NoopProtectedEffectAudit),
        },
        session,
        ContextBudget::calculate(200_000, "", 0, 16_000),
        InterruptHandle::new(),
    )
    .build();
    Fixture {
        runner: AgentLoopRunner::new(params),
        frontend,
        input_tx,
        temp,
    }
}

pub(super) fn envelope(source: MessageSource, text: &str) -> Envelope {
    Envelope::new(source, "runtime-coverage", text)
}

pub(super) fn terminal(
    session_id: &str,
    run_id: &str,
    revision: u64,
) -> WorkflowTerminalNotification {
    WorkflowTerminalNotification {
        delivery_id: WorkflowTerminalDeliveryId::new(
            session_id,
            WorkflowRunId::new(run_id),
            revision,
        ),
        state: WorkflowRunState::Succeeded,
        run_goal: "cover terminal delivery".into(),
        outcome: WorkflowTerminalOutcome::Succeeded {
            result: "done".into(),
        },
        content: "workflow done".into(),
    }
}
