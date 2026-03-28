//! SessionController: observation + control + multi-agent connections.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::{mpsc, watch};

use loopal_ipc::connection::Connection;
use loopal_protocol::{
    AgentEvent, AgentMode, ControlCommand, InterruptSignal, UserContent, UserQuestionResponse,
};

use crate::controller_ops::ControlBackend;
use crate::event_handler;
use crate::state::SessionState;
use loopal_agent_hub::{AgentHub, LocalChannels};

/// External handle — cheaply cloneable, shareable across consumers.
#[derive(Clone)]
pub struct SessionController {
    state: Arc<Mutex<SessionState>>,
    pub(crate) backend: Arc<ControlBackend>,
    connections: Arc<tokio::sync::Mutex<AgentHub>>,
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
            connections: Arc::new(tokio::sync::Mutex::new(AgentHub::noop())),
        }
    }

    /// Create with Hub Connection (production mode — all agents via Hub).
    pub fn with_hub(
        model: String,
        mode: String,
        hub_conn: Arc<Connection>,
        hub: Arc<tokio::sync::Mutex<AgentHub>>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(SessionState::new(model, mode))),
            backend: Arc::new(ControlBackend::Hub(hub_conn)),
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

    // === Root agent control ===

    pub fn interrupt(&self) {
        tracing::debug!("session: interrupt signaled");
        self.backend.interrupt();
    }

    pub fn enqueue_message(&self, content: UserContent) -> Option<UserContent> {
        let mut state = self.lock();
        state.inbox.push(content);
        crate::controller_ops::try_forward_from_inbox(&mut state)
    }

    pub async fn approve_permission(&self) {
        {
            self.lock().pending_permission = None;
        }
        self.backend.approve_permission().await;
    }

    pub async fn deny_permission(&self) {
        {
            self.lock().pending_permission = None;
        }
        self.backend.deny_permission().await;
    }

    pub async fn answer_question(&self, answers: Vec<String>) {
        {
            self.lock().pending_question = None;
        }
        self.backend.answer_question(answers).await;
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
        self.backend
            .send_control(ControlCommand::ModeSwitch(mode))
            .await;
    }

    pub async fn switch_model(&self, model: String) {
        {
            let mut s = self.lock();
            s.model = model.clone();
            crate::helpers::push_system_msg(&mut s, &format!("Switched model to: {model}"));
        }
        self.backend
            .send_control(ControlCommand::ModelSwitch(model))
            .await;
    }

    pub async fn switch_thinking(&self, config_json: String) {
        let label = crate::helpers::thinking_label_from_json(&config_json);
        {
            let mut s = self.lock();
            s.thinking_config = label.clone();
            crate::helpers::push_system_msg(&mut s, &format!("Switched thinking to: {label}"));
        }
        self.backend
            .send_control(ControlCommand::ThinkingSwitch(config_json))
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
        self.backend.send_control(ControlCommand::Clear).await;
    }

    pub async fn compact(&self) {
        self.backend.send_control(ControlCommand::Compact).await;
    }

    pub async fn rewind(&self, turn_index: usize) {
        self.backend
            .send_control(ControlCommand::Rewind { turn_index })
            .await;
    }

    // === Event handling ===

    pub fn handle_event(&self, event: AgentEvent) -> Option<UserContent> {
        let mut state = self.lock();
        event_handler::apply_event(&mut state, event)
    }
}
