use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::agent_input::AgentInput;
use crate::frontend::traits::{AgentFrontend, EventEmitter};
use loopal_error::Result;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_protocol::QualifiedAddress;
use loopal_protocol::{AgentEvent, AgentEventPayload};
use loopal_tool_api::PermissionDecision;

use super::emitter::ChannelEventEmitter;
use super::permission_handler::PermissionHandler;
use super::question_handler::QuestionHandler;

pub struct UnifiedFrontend {
    agent_name: Option<QualifiedAddress>,
    event_tx: mpsc::Sender<AgentEvent>,
    mailbox_rx: Mutex<mpsc::Receiver<Envelope>>,
    control_rx: Mutex<mpsc::Receiver<ControlCommand>>,
    cancel_token: Option<CancellationToken>,
    permission_handler: Box<dyn PermissionHandler>,
    question_handler: Box<dyn QuestionHandler>,
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
        }
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
        if let Some(ref token) = self.cancel_token {
            tokio::select! {
                env = mbox.recv() => env.map(AgentInput::Message),
                cmd = ctrl.recv() => cmd.map(AgentInput::Control),
                () = token.cancelled() => {
                    info!("cancellation triggered in unified frontend");
                    None
                }
            }
        } else {
            tokio::select! {
                env = mbox.recv() => env.map(AgentInput::Message),
                cmd = ctrl.recv() => cmd.map(AgentInput::Control),
            }
        }
    }

    async fn request_permission(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionDecision {
        let outcome = self.permission_handler.decide(id, name, input).await;
        let (decision, payload) = super::dispatch::into_permission_decided(name, outcome);
        if let Err(e) = self.emit(payload).await {
            tracing::error!(ctx = "unified::permission_decided", error = %e, "event emit failed");
        }
        decision
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

    fn try_emit(&self, payload: AgentEventPayload) -> bool {
        let event = AgentEvent::for_agent(self.agent_name.clone(), payload);
        self.event_tx.try_send(event).is_ok()
    }
}
