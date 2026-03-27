//! SessionController: observation + control + multi-agent connections.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::{mpsc, watch};

use loopal_protocol::{
    AgentEvent, AgentMode, ControlCommand, InterruptSignal, UserContent, UserQuestionResponse,
};

use crate::event_handler;
use crate::inbox::try_forward_inbox;
use crate::state::SessionState;
use loopal_agent_hub::{AgentHub, PrimaryConn};

/// External handle — cheaply cloneable, shareable across consumers.
#[derive(Clone)]
pub struct SessionController {
    state: Arc<Mutex<SessionState>>,
    primary: Arc<PrimaryConn>,
    connections: Arc<tokio::sync::Mutex<AgentHub>>,
}

impl SessionController {
    /// Create with individual channels (backward compat for tests).
    pub fn new(
        model: String,
        mode: String,
        control_tx: mpsc::Sender<ControlCommand>,
        permission_tx: mpsc::Sender<bool>,
        question_tx: mpsc::Sender<UserQuestionResponse>,
        interrupt: InterruptSignal,
        interrupt_tx: Arc<watch::Sender<u64>>,
    ) -> Self {
        let primary = PrimaryConn {
            control_tx,
            permission_tx,
            question_tx,
            mailbox_tx: None,
            interrupt,
            interrupt_tx,
        };
        Self {
            state: Arc::new(Mutex::new(SessionState::new(model, mode))),
            primary: Arc::new(primary),
            connections: Arc::new(tokio::sync::Mutex::new(AgentHub::noop())),
        }
    }

    /// Create with structured primary connection + agent hub.
    pub fn with_primary(
        model: String,
        mode: String,
        primary: PrimaryConn,
        hub: Arc<tokio::sync::Mutex<AgentHub>>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(SessionState::new(model, mode))),
            primary: Arc::new(primary),
            connections: hub,
        }
    }

    // === Observability ===

    pub fn lock(&self) -> MutexGuard<'_, SessionState> {
        self.state.lock().expect("session state lock poisoned")
    }

    pub(crate) fn connections(&self) -> &Arc<tokio::sync::Mutex<AgentHub>> {
        &self.connections
    }

    pub(crate) fn primary(&self) -> &PrimaryConn {
        &self.primary
    }

    // === Root agent control ===

    pub fn interrupt(&self) {
        tracing::debug!("session: interrupt signaled");
        self.primary.interrupt.signal();
        self.primary
            .interrupt_tx
            .send_modify(|v| *v = v.wrapping_add(1));
    }

    pub fn enqueue_message(&self, content: UserContent) -> Option<UserContent> {
        let mut state = self.lock();
        state.inbox.push(content);
        try_forward_inbox(&mut state)
    }

    pub async fn approve_permission(&self) {
        {
            self.lock().pending_permission = None;
        }
        let _ = self.primary.permission_tx.send(true).await;
    }

    pub async fn deny_permission(&self) {
        {
            self.lock().pending_permission = None;
        }
        let _ = self.primary.permission_tx.send(false).await;
    }

    pub async fn answer_question(&self, answers: Vec<String>) {
        {
            self.lock().pending_question = None;
        }
        let _ = self
            .primary
            .question_tx
            .send(UserQuestionResponse { answers })
            .await;
    }

    pub async fn switch_mode(&self, mode: AgentMode) {
        {
            let mut s = self.lock();
            s.mode = match mode {
                AgentMode::Plan => "plan",
                AgentMode::Act => "act",
            }
            .to_string();
        }
        let _ = self
            .primary
            .control_tx
            .send(ControlCommand::ModeSwitch(mode))
            .await;
    }

    pub async fn switch_model(&self, model: String) {
        {
            let mut s = self.lock();
            s.model = model.clone();
            crate::helpers::push_system_msg(&mut s, &format!("Switched model to: {model}"));
        }
        let _ = self
            .primary
            .control_tx
            .send(ControlCommand::ModelSwitch(model))
            .await;
    }

    pub async fn switch_thinking(&self, config_json: String) {
        let label = crate::helpers::thinking_label_from_json(&config_json);
        {
            let mut s = self.lock();
            s.thinking_config = label.clone();
            crate::helpers::push_system_msg(&mut s, &format!("Switched thinking to: {label}"));
        }
        let _ = self
            .primary
            .control_tx
            .send(ControlCommand::ThinkingSwitch(config_json))
            .await;
    }

    pub async fn clear(&self) {
        {
            let mut s = self.lock();
            s.messages.clear();
            s.inbox.clear();
            s.streaming_text.clear();
            s.turn_count = 0;
            s.input_tokens = 0;
            s.output_tokens = 0;
            s.cache_creation_tokens = 0;
            s.cache_read_tokens = 0;
            s.retry_banner = None;
            s.reset_timer();
        }
        let _ = self.primary.control_tx.send(ControlCommand::Clear).await;
    }

    pub async fn compact(&self) {
        let _ = self.primary.control_tx.send(ControlCommand::Compact).await;
    }

    pub async fn rewind(&self, turn_index: usize) {
        let _ = self
            .primary
            .control_tx
            .send(ControlCommand::Rewind { turn_index })
            .await;
    }

    // === Event handling ===

    pub fn handle_event(&self, event: AgentEvent) -> Option<UserContent> {
        let mut state = self.lock();
        event_handler::apply_event(&mut state, event)
    }
}
