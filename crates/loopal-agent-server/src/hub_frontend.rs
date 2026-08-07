use std::sync::Arc;

use async_trait::async_trait;

use loopal_error::Result;
use loopal_protocol::{AgentEventPayload, Question, UserQuestionResponse};
use loopal_runtime::agent_input::AgentInput;
use loopal_runtime::frontend::permission_handler::PermissionHandler;
use loopal_runtime::frontend::question_handler::QuestionHandler;
use loopal_runtime::frontend::traits::{AgentFrontend, EventEmitter};
use loopal_tool_api::PermissionDecision;
use tokio_util::sync::CancellationToken;

use crate::hub_broadcaster::HubBroadcaster;
use crate::hub_input_receiver::HubInputReceiver;
use crate::ipc_handlers::{
    IpcPermissionHandler, IpcQuestionHandler, SessionRef, request_plan_approval,
};
use crate::session_hub::SharedSession;

pub struct HubFrontend {
    session: SessionRef,
    broadcaster: HubBroadcaster,
    input: HubInputReceiver,
    permission_handler: Box<dyn PermissionHandler>,
    question_handler: Box<dyn QuestionHandler>,
}

impl HubFrontend {
    pub fn new(
        session: SessionRef,
        input_rx: tokio::sync::mpsc::Receiver<AgentInput>,
        agent_name: Option<String>,
        interrupt_rx: tokio::sync::watch::Receiver<u64>,
        shutdown: CancellationToken,
        permission_handler: Box<dyn PermissionHandler>,
        question_handler: Box<dyn QuestionHandler>,
    ) -> Self {
        let qa = agent_name.map(loopal_protocol::QualifiedAddress::local);
        let broadcaster = HubBroadcaster::new(session.clone(), qa);
        Self::new_with_broadcaster(
            session,
            broadcaster,
            input_rx,
            interrupt_rx,
            shutdown,
            permission_handler,
            question_handler,
        )
    }

    pub(crate) fn new_with_broadcaster(
        session: SessionRef,
        broadcaster: HubBroadcaster,
        input_rx: tokio::sync::mpsc::Receiver<AgentInput>,
        interrupt_rx: tokio::sync::watch::Receiver<u64>,
        shutdown: CancellationToken,
        permission_handler: Box<dyn PermissionHandler>,
        question_handler: Box<dyn QuestionHandler>,
    ) -> Self {
        Self {
            session,
            broadcaster,
            input: HubInputReceiver::new(input_rx, interrupt_rx, shutdown),
            permission_handler,
            question_handler,
        }
    }

    pub async fn replace_session(&self, session: Arc<SharedSession>) {
        self.broadcaster.replace_session(session).await;
    }

    #[doc(hidden)]
    pub fn new_for_test(
        session: Arc<SharedSession>,
        input_rx: tokio::sync::mpsc::Receiver<AgentInput>,
        agent_name: Option<String>,
        interrupt_rx: tokio::sync::watch::Receiver<u64>,
    ) -> Self {
        let session_ref: SessionRef = Arc::new(tokio::sync::RwLock::new(session));
        let perm: Box<dyn PermissionHandler> =
            Box::new(IpcPermissionHandler::new(session_ref.clone()));
        let q: Box<dyn QuestionHandler> = Box::new(IpcQuestionHandler::new(session_ref.clone()));
        Self::new(
            session_ref,
            input_rx,
            agent_name,
            interrupt_rx,
            CancellationToken::new(),
            perm,
            q,
        )
    }
}

#[async_trait]
impl AgentFrontend for HubFrontend {
    async fn emit(&self, payload: AgentEventPayload) -> Result<()> {
        self.broadcaster.broadcast(payload).await
    }

    async fn emit_in_turn(&self, payload: AgentEventPayload) -> Result<()> {
        self.broadcaster.broadcast_in_turn(payload).await
    }

    async fn recv_input(&self) -> Option<AgentInput> {
        self.input.next().await
    }

    async fn try_recv_input(
        &self,
    ) -> std::result::Result<AgentInput, tokio::sync::mpsc::error::TryRecvError> {
        self.input.try_next().await
    }

    async fn request_permission(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> PermissionDecision {
        let outcome = self.permission_handler.decide(id, name, input).await;
        let (decision, payload) = loopal_runtime::frontend::into_permission_decided(name, outcome);
        let _ = self.broadcaster.broadcast(payload).await;
        decision
    }

    fn event_emitter(&self) -> Box<dyn EventEmitter> {
        Box::new(self.broadcaster.clone())
    }

    async fn ask_user(&self, questions: Vec<Question>) -> UserQuestionResponse {
        let n = questions.len() as u32;
        let outcome = self.question_handler.ask(questions).await;
        let (response, payload) = loopal_runtime::frontend::into_question_decided(n, outcome);
        let _ = self.broadcaster.broadcast(payload).await;
        response
    }

    async fn request_plan_approval(
        &self,
        plan_content: &str,
        plan_path: &str,
    ) -> loopal_runtime::frontend::traits::PlanApproval {
        request_plan_approval(&self.session, plan_content, plan_path).await
    }

    fn try_emit(&self, payload: AgentEventPayload) -> bool {
        self.broadcaster.try_broadcast(payload)
    }

    async fn drain_pending(&self) -> Vec<AgentInput> {
        self.input.drain().await
    }
}
