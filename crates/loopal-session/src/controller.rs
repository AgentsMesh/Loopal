//! SessionController: observation + control + multi-agent connections.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::{mpsc, watch};

use loopal_protocol::{
    AgentEvent, ControlCommand, InterruptSignal, UserContent, UserQuestionResponse,
};

use crate::controller_ops::ControlBackend;
use crate::event_handler;
use crate::state::SessionState;
use loopal_agent_hub::{Hub, HubClient, LocalChannels};

/// External handle — cheaply cloneable, shareable across consumers.
#[derive(Clone)]
pub struct SessionController {
    state: Arc<Mutex<SessionState>>,
    pub(crate) backend: Arc<ControlBackend>,
    connections: Arc<tokio::sync::Mutex<Hub>>,
}

impl SessionController {
    /// Create with in-process channels (for unit tests — no real Hub).
    pub fn new(
        model: String,
        mode: String,
        control_tx: mpsc::Sender<ControlCommand>,
        permission_tx: mpsc::Sender<bool>,
        question_tx: mpsc::Sender<UserQuestionResponse>,
        interrupt: InterruptSignal,
        interrupt_tx: Arc<watch::Sender<u64>>,
    ) -> Self {
        let channels = LocalChannels {
            control_tx,
            permission_tx,
            question_tx,
            mailbox_tx: None,
            interrupt,
            interrupt_tx,
        };
        Self {
            state: Arc::new(Mutex::new(SessionState::new(model, mode))),
            backend: Arc::new(ControlBackend::Local(Arc::new(channels))),
            connections: Arc::new(tokio::sync::Mutex::new(Hub::noop())),
        }
    }

    /// Create with Hub Connection (production mode — all agents via Hub).
    pub fn with_hub(
        model: String,
        mode: String,
        client: Arc<HubClient>,
        hub: Arc<tokio::sync::Mutex<Hub>>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(SessionState::new(model, mode))),
            backend: Arc::new(ControlBackend::Hub(client)),
            connections: hub,
        }
    }

    /// Acquire the session state lock. Panics if the lock is poisoned.
    pub fn lock(&self) -> MutexGuard<'_, SessionState> {
        self.state.lock().expect("session state lock poisoned")
    }

    pub(crate) fn connections(&self) -> &Arc<tokio::sync::Mutex<Hub>> {
        &self.connections
    }

    pub(crate) fn active_target(&self) -> String {
        self.lock().active_view.clone()
    }

    /// Interrupt the currently viewed agent.
    pub fn interrupt(&self) {
        let target = self.active_target();
        tracing::info!(target = %target, "session: interrupt signaled");
        self.backend.interrupt_target(&target);
    }

    /// Interrupt a specific named agent (e.g., to terminate from agent panel).
    pub fn interrupt_agent(&self, name: &str) {
        tracing::debug!(agent = %name, "session: interrupt agent");
        self.backend.interrupt_target(name);
    }

    pub fn enqueue_message(&self, content: UserContent) -> Option<UserContent> {
        let mut state = self.lock();
        state.inbox.push(content);
        crate::controller_ops::try_forward_from_inbox(&mut state)
    }

    pub async fn approve_permission(&self) {
        let relay_id = {
            let mut state = self.lock();
            let conv = state.active_conversation_mut();
            let relay_id = conv
                .pending_permission
                .as_ref()
                .and_then(|p| p.relay_request_id);
            conv.pending_permission = None;
            relay_id
        };
        self.backend.approve_permission(relay_id).await;
    }

    pub async fn deny_permission(&self) {
        let relay_id = {
            let mut state = self.lock();
            let conv = state.active_conversation_mut();
            let relay_id = conv
                .pending_permission
                .as_ref()
                .and_then(|p| p.relay_request_id);
            conv.pending_permission = None;
            relay_id
        };
        self.backend.deny_permission(relay_id).await;
    }

    pub async fn answer_question(&self, answers: Vec<String>) {
        let relay_id = {
            let mut state = self.lock();
            let conv = state.active_conversation_mut();
            let relay_id = conv
                .pending_question
                .as_ref()
                .and_then(|q| q.relay_request_id);
            conv.pending_question = None;
            relay_id
        };
        self.backend.answer_question(answers, relay_id).await;
    }

    // === Direct relay responses (for auto-approve mode) ===

    /// Auto-approve a permission request using the relay ID directly.
    /// Bypasses pending_permission state — avoids race with event broadcast.
    pub async fn auto_approve_permission(&self, relay_id: i64) {
        self.backend.approve_permission(Some(relay_id)).await;
    }

    /// Auto-answer a question using the relay ID directly.
    pub async fn auto_answer_question(&self, relay_id: i64, answers: Vec<String>) {
        self.backend.answer_question(answers, Some(relay_id)).await;
    }

    // === Event handling ===

    pub fn handle_event(&self, event: AgentEvent) -> Option<UserContent> {
        let mut state = self.lock();
        event_handler::apply_event(&mut state, event)
    }

    /// Set the root session ID (for sub-agent ref persistence).
    pub fn set_root_session_id(&self, session_id: &str) {
        self.lock().root_session_id = Some(session_id.to_string());
    }

    /// Drain pending sub-agent refs that need to be persisted.
    /// Returns `(root_session_id, refs)`. The caller is responsible for
    /// writing them to disk via `SessionManager::add_sub_agent`.
    pub fn drain_pending_sub_agent_refs(
        &self,
    ) -> Option<(String, Vec<crate::state::PendingSubAgentRef>)> {
        let mut state = self.lock();
        if state.pending_sub_agent_refs.is_empty() {
            return None;
        }
        let refs = std::mem::take(&mut state.pending_sub_agent_refs);
        let root_id = state.root_session_id.clone()?;
        Some((root_id, refs))
    }
}
