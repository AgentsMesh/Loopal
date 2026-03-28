/// Observable session state — pure data, no channels.
use std::time::{Duration, Instant};

use indexmap::IndexMap;

use loopal_protocol::ObservableAgentState;

use crate::inbox::Inbox;
use crate::message_log::{MessageFeed, MessageLogEntry};
use crate::types::{DisplayMessage, PendingPermission, PendingQuestion};

/// Enhanced agent view state with full observability.
#[derive(Debug, Clone, Default)]
pub struct AgentViewState {
    /// Rich observable state (status, tokens, model, mode, etc.).
    pub observable: ObservableAgentState,
    /// Per-agent message log (sent/received messages).
    pub message_log: Vec<MessageLogEntry>,
    /// Timestamp when the agent was first observed (for elapsed display).
    pub started_at: Option<Instant>,
    /// Parent agent name (None for root-spawned agents).
    pub parent: Option<String>,
    /// Names of agents spawned by this agent.
    pub children: Vec<String>,
}

impl AgentViewState {
    /// Elapsed time since the agent was first observed.
    pub fn elapsed(&self) -> Duration {
        self.started_at.map_or(Duration::ZERO, |t| t.elapsed())
    }
}

/// All observable state of a session, protected by a Mutex in SessionController.
pub struct SessionState {
    // === Observable state ===
    pub messages: Vec<DisplayMessage>,
    pub streaming_text: String,
    pub agent_idle: bool,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub context_window: u32,
    pub cache_creation_tokens: u32,
    pub cache_read_tokens: u32,
    pub thinking_tokens: u32,
    pub streaming_thinking: String,
    pub thinking_active: bool,
    pub turn_count: u32,
    pub model: String,
    pub mode: String,
    /// Current thinking config label for display ("auto", "low", "medium", "high", "disabled").
    pub thinking_config: String,
    pub pending_permission: Option<PendingPermission>,
    pub pending_question: Option<PendingQuestion>,
    /// Transient retry error banner — shown in TUI, not in message history.
    pub retry_banner: Option<String>,
    // === Agent tracking (observation plane) ===
    pub agents: IndexMap<String, AgentViewState>,
    pub focused_agent: Option<String>,
    pub message_feed: MessageFeed,
    // === Turn timer ===
    turn_start: Option<Instant>,
    last_turn_duration: Duration,
    // === Interaction state ===
    pub inbox: Inbox,
}

impl SessionState {
    pub fn new(model: String, mode: String) -> Self {
        Self {
            messages: Vec::new(),
            streaming_text: String::new(),
            agent_idle: false,
            input_tokens: 0,
            output_tokens: 0,
            context_window: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            thinking_tokens: 0,
            streaming_thinking: String::new(),
            thinking_active: false,
            turn_count: 0,
            model,
            mode,
            thinking_config: "auto".to_string(),
            pending_permission: None,
            pending_question: None,
            retry_banner: None,
            agents: IndexMap::new(),
            focused_agent: None,
            message_feed: MessageFeed::new(200),
            turn_start: None,
            last_turn_duration: Duration::ZERO,
            inbox: Inbox::new(),
        }
    }

    /// Total token count for context usage display.
    pub fn token_count(&self) -> u32 {
        self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens
    }

    /// Current turn working duration.
    pub fn turn_elapsed(&self) -> Duration {
        match self.turn_start {
            Some(start) => start.elapsed(),
            None => self.last_turn_duration,
        }
    }

    /// Mark the start of a new turn (agent begins working).
    pub fn begin_turn(&mut self) {
        if self.turn_start.is_none() {
            self.turn_start = Some(Instant::now());
        }
    }

    /// Mark the end of a turn (agent became idle).
    pub fn end_turn(&mut self) {
        if let Some(start) = self.turn_start.take() {
            self.last_turn_duration = start.elapsed();
        }
    }

    /// Reset the turn timer (e.g., after /clear).
    pub fn reset_timer(&mut self) {
        self.turn_start = None;
        self.last_turn_duration = Duration::ZERO;
    }
}
