//! SessionController: Arc<Mutex<SessionState>> + channel handles.
//!
//! Pure observation layer — tracks state and forwards control commands.
//! Does NOT hold MessageRouter; message routing is the TUI's responsibility.

use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::mpsc;

use loopal_protocol::AgentMode;
use loopal_protocol::ControlCommand;
use loopal_protocol::AgentEvent;
use loopal_protocol::UserQuestionResponse;

use crate::event_handler;
use crate::state::SessionState;
use crate::types::DisplayMessage;

/// External handle — cheaply cloneable, shareable across consumers.
///
/// Provides observation (state reading) and control (mode/model switch, clear).
/// Message routing to agents is handled externally (by TUI or test harness).
#[derive(Clone)]
pub struct SessionController {
    state: Arc<Mutex<SessionState>>,
    control_tx: mpsc::Sender<ControlCommand>,
    permission_tx: mpsc::Sender<bool>,
    question_tx: mpsc::Sender<UserQuestionResponse>,
}

impl SessionController {
    pub fn new(
        model: String,
        mode: String,
        control_tx: mpsc::Sender<ControlCommand>,
        permission_tx: mpsc::Sender<bool>,
        question_tx: mpsc::Sender<UserQuestionResponse>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(SessionState::new(model, mode))),
            control_tx,
            permission_tx,
            question_tx,
        }
    }

    // === Observability ===

    /// Lock the state for reading. All reads go through this guard.
    pub fn lock(&self) -> MutexGuard<'_, SessionState> {
        self.state.lock().expect("session state lock poisoned")
    }

    // === Interaction (control plane only) ===

    /// Push a message into inbox. Returns Some(text) if it should be forwarded.
    ///
    /// Caller is responsible for actually routing the message to the agent
    /// (e.g., via `MessageRouter::route()`).
    pub fn enqueue_message(&self, text: String) -> Option<String> {
        let mut state = self.lock();
        state.inbox.push(text);
        event_handler::try_forward_inbox(&mut state)
    }

    /// Approve the current pending permission request.
    pub async fn approve_permission(&self) {
        { self.lock().pending_permission = None; }
        let _ = self.permission_tx.send(true).await;
    }

    /// Deny the current pending permission request.
    pub async fn deny_permission(&self) {
        { self.lock().pending_permission = None; }
        let _ = self.permission_tx.send(false).await;
    }

    /// Submit answers to a pending question (AskUser tool).
    pub async fn answer_question(&self, answers: Vec<String>) {
        { self.lock().pending_question = None; }
        let _ = self.question_tx.send(UserQuestionResponse { answers }).await;
    }

    /// Switch agent mode (Plan / Act).
    pub async fn switch_mode(&self, mode: AgentMode) {
        {
            let mut state = self.lock();
            state.mode = match mode {
                AgentMode::Plan => "plan",
                AgentMode::Act => "act",
            }.to_string();
        }
        let _ = self.control_tx.send(ControlCommand::ModeSwitch(mode)).await;
    }

    /// Switch the LLM model.
    pub async fn switch_model(&self, model: String) {
        {
            let mut state = self.lock();
            state.model = model.clone();
            state.messages.push(DisplayMessage {
                role: "system".to_string(),
                content: format!("Switched model to: {model}"),
                tool_calls: Vec::new(),
            });
        }
        let _ = self.control_tx.send(ControlCommand::ModelSwitch(model)).await;
    }

    /// Clear all messages, inbox, streaming buffer and counters.
    pub async fn clear(&self) {
        {
            let mut state = self.lock();
            state.messages.clear();
            state.inbox.clear();
            state.streaming_text.clear();
            state.turn_count = 0;
            state.input_tokens = 0;
            state.output_tokens = 0;
            state.cache_creation_tokens = 0;
            state.cache_read_tokens = 0;
            state.reset_timer();
        }
        let _ = self.control_tx.send(ControlCommand::Clear).await;
    }

    /// Request context compaction.
    pub async fn compact(&self) {
        let _ = self.control_tx.send(ControlCommand::Compact).await;
    }

    /// Rewind conversation to the given turn index.
    /// The runtime will truncate messages and emit a Rewound event.
    pub async fn rewind(&self, turn_index: usize) {
        let _ = self.control_tx.send(ControlCommand::Rewind { turn_index }).await;
    }

    /// Pop the last inbox message for editing. Returns None if empty.
    pub fn pop_inbox_to_edit(&self) -> Option<String> {
        self.lock().inbox.pop_back()
    }

    /// Push a system message into the display.
    pub fn push_system_message(&self, content: String) {
        self.lock().messages.push(DisplayMessage {
            role: "system".to_string(),
            content,
            tool_calls: Vec::new(),
        });
    }

    /// Load historical display messages (e.g., after session resume).
    pub fn load_display_history(&self, display_msgs: Vec<DisplayMessage>) {
        self.lock().messages = display_msgs;
    }

    // === Event handling ===

    /// Process an AgentEvent by updating internal state.
    /// Returns `Some(text)` if an inbox message should be forwarded.
    pub fn handle_event(&self, event: AgentEvent) -> Option<String> {
        let mut state = self.lock();
        event_handler::apply_event(&mut state, event)
    }
}
