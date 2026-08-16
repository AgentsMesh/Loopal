use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::agent_input::AgentInput;
use crate::frontend::traits::{AgentFrontend, EventEmitter, PlanApproval};
use loopal_error::Result;
use loopal_protocol::{
    AgentEvent, AgentEventPayload, ControlCommand, Envelope, PermissionIntentRequest,
    QualifiedAddress,
};
use loopal_tool_api::PermissionDecision;

use super::emitter::ChannelEventEmitter;
use super::permission_handler::{PermissionHandler, PermissionOutcome};
use super::question_handler::QuestionHandler;

pub struct UnifiedFrontend {
    agent_name: Option<QualifiedAddress>,
    event_tx: mpsc::Sender<AgentEvent>,
    mailbox_rx: Mutex<mpsc::Receiver<Envelope>>,
    control_rx: Mutex<mpsc::Receiver<ControlCommand>>,
    cancel_token: Option<CancellationToken>,
    permission_handler: Box<dyn PermissionHandler>,
    question_handler: Box<dyn QuestionHandler>,
    plan_approval: PlanApproval,
}

impl UnifiedFrontend {
    pub fn new(
        agent_name: Option<String>,
        event_tx: mpsc::Sender<AgentEvent>,
        mailbox_rx: mpsc::Receiver<Envelope>,
        control_rx: mpsc::Receiver<ControlCommand>,
        cancel_token: Option<CancellationToken>,
        permission_handler: Box<dyn PermissionHandler>,
        question_handler: Box<dyn QuestionHandler>,
    ) -> Self {
        Self {
            agent_name: agent_name.map(QualifiedAddress::local),
            event_tx,
            mailbox_rx: Mutex::new(mailbox_rx),
            control_rx: Mutex::new(control_rx),
            cancel_token,
            permission_handler,
            question_handler,
            plan_approval: PlanApproval::Reject,
        }
    }

    pub fn with_plan_approval(mut self, approval: PlanApproval) -> Self {
        self.plan_approval = approval;
        self
    }
}

#[async_trait]
impl AgentFrontend for UnifiedFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        let event = AgentEvent::for_agent(self.agent_name.clone(), payload);
        self.event_tx.send(event).await.map_err(|e| {
            warn!(error = %e, "event channel closed");
            loopal_error::LoopalError::Other("event channel closed".into())
        })
    }

    async fn emit_in_turn(&self, payload: AgentEventPayload) -> Result<()> {
        // Build envelope via for_agent_in_turn so missing scope_turn panics
        // at envelope construction (not at a later silent turn_id=0).
        let event = AgentEvent::for_agent_in_turn(self.agent_name.clone(), payload);
        self.event_tx.send(event).await.map_err(|e| {
            warn!(error = %e, "event channel closed");
            loopal_error::LoopalError::Other("event channel closed".into())
        })
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        let mut mbox = self.mailbox_rx.lock().await;
        let mut ctrl = self.control_rx.lock().await;
        loop {
            let mailbox_drained = mbox.is_closed() && mbox.is_empty();
            let control_drained = ctrl.is_closed() && ctrl.is_empty();
            if mailbox_drained && control_drained {
                return None;
            }

            tokio::select! {
                env = mbox.recv(), if !mailbox_drained => {
                    if let Some(env) = env {
                        return Some(AgentInput::Message(env));
                    }
                }
                cmd = ctrl.recv(), if !control_drained => {
                    if let Some(cmd) = cmd {
                        return Some(AgentInput::Control(cmd));
                    }
                }
                () = async {
                    match self.cancel_token.as_ref() {
                        Some(token) => token.cancelled().await,
                        None => std::future::pending().await,
                    }
                } => {
                    info!("cancellation triggered in unified frontend");
                    return None;
                }
            }
        }
    }

    async fn try_recv_input(&self) -> std::result::Result<AgentInput, mpsc::error::TryRecvError> {
        if self
            .cancel_token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(mpsc::error::TryRecvError::Disconnected);
        }

        let mut mbox = self.mailbox_rx.lock().await;
        let mut ctrl = self.control_rx.lock().await;

        // Control wins same-boundary races so Suspend closes the continuation
        // gate before automatic goal work can start another provider request.
        let control_disconnected = match ctrl.try_recv() {
            Ok(command) => return Ok(AgentInput::Control(command)),
            Err(mpsc::error::TryRecvError::Empty) => false,
            Err(mpsc::error::TryRecvError::Disconnected) => true,
        };
        match mbox.try_recv() {
            Ok(envelope) => Ok(AgentInput::Message(envelope)),
            Err(mpsc::error::TryRecvError::Empty) => Err(mpsc::error::TryRecvError::Empty),
            Err(mpsc::error::TryRecvError::Disconnected) if control_disconnected => {
                Err(mpsc::error::TryRecvError::Disconnected)
            }
            Err(mpsc::error::TryRecvError::Disconnected) => Err(mpsc::error::TryRecvError::Empty),
        }
    }

    async fn request_permission(&self, request: &PermissionIntentRequest) -> PermissionDecision {
        self.request_permission_outcome(request).await.decision
    }

    async fn request_permission_outcome(
        &self,
        request: &PermissionIntentRequest,
    ) -> PermissionOutcome {
        let outcome = self.permission_handler.decide(request).await;
        let (decision, payload) =
            super::dispatch::into_permission_decided(&request.tool_name, outcome.clone());
        if let Err(e) = self.emit(payload).await {
            tracing::error!(ctx = "unified::permission_decided", error = %e, "event emit failed");
        }
        let _ = decision;
        outcome
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(ChannelEventEmitter::new(
            self.event_tx.clone(),
            self.agent_name.clone(),
        ))
    }

    async fn drain_pending(&self) -> Vec<AgentInput> {
        let mut mbox = self.mailbox_rx.lock().await;
        let mut ctrl = self.control_rx.lock().await;
        let mut inputs = Vec::new();
        while let Ok(env) = mbox.try_recv() {
            inputs.push(AgentInput::Message(env));
        }
        while let Ok(cmd) = ctrl.try_recv() {
            inputs.push(AgentInput::Control(cmd));
        }
        inputs
    }

    async fn ask_user(
        &self,
        questions: Vec<loopal_protocol::Question>,
    ) -> loopal_protocol::UserQuestionResponse {
        let n = questions.len() as u32;
        let outcome = self.question_handler.ask(questions).await;
        let (response, payload) = super::dispatch::into_question_decided(n, outcome);
        if let Err(e) = self.emit(payload).await {
            tracing::error!(ctx = "unified::question_decided", error = %e, "event emit failed");
        }
        response
    }

    async fn request_plan_approval(&self, _plan_content: &str, _plan_path: &str) -> PlanApproval {
        self.plan_approval.clone()
    }

    fn try_emit(&self, payload: AgentEventPayload) -> bool {
        let event = AgentEvent::for_agent(self.agent_name.clone(), payload);
        self.event_tx.try_send(event).is_ok()
    }
}
