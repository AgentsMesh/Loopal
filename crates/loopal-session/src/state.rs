/// Observable session state — pure data, no channels.
///
/// All agents (including root) share the same `AgentViewState` type
/// in the `agents` map. `active_view` determines which agent's conversation
/// is rendered and receives user input.
use std::time::Instant;

use indexmap::IndexMap;
use loopal_protocol::{
    AgentStatus, BgTaskDetail, CronJobSnapshot, McpServerSnapshot, TaskSnapshot,
};

/// Name of the root agent in the agents map.
pub const ROOT_AGENT: &str = "main";

use loopal_protocol::ObservableAgentState;

use crate::agent_conversation::AgentConversation;
use crate::message_log::{MessageFeed, MessageLogEntry};

/// Enhanced agent view state with full observability.
#[derive(Debug, Default)]
pub struct AgentViewState {
    /// Rich observable state (status, tokens, model, mode, etc.).
    pub observable: ObservableAgentState,
    /// Full conversation state (messages, streaming, pending interactions).
    pub conversation: AgentConversation,
    /// Per-agent message log (sent/received inter-agent messages).
    pub message_log: Vec<MessageLogEntry>,
    /// Timestamp when the agent was first observed (for elapsed display).
    pub started_at: Option<Instant>,
    /// Parent agent name (None for root).
    pub parent: Option<String>,
    /// Names of agents spawned by this agent.
    pub children: Vec<String>,
    /// Sub-agent's own session storage ID (for resume/persistence).
    pub session_id: Option<String>,
}

/// All observable state of a session, protected by a Mutex in SessionController.
pub struct SessionState {
    // === All agents (including root "main") ===
    pub agents: IndexMap<String, AgentViewState>,
    /// Which agent's conversation is displayed and receives input. Default: "main".
    pub active_view: String,
    // === Session-level display cache (synced from active agent's observable) ===
    /// Model name shown in status bar. Updated on ModelSwitch or ModeChanged.
    pub model: String,
    /// Current mode label ("act" / "plan"). Updated when active agent changes mode.
    pub mode: String,
    /// Current thinking config label for display.
    pub thinking_config: String,
    /// Root session ID for persisting sub-agent references.
    pub root_session_id: Option<String>,
    // === Observation plane ===
    pub message_feed: MessageFeed,
    // === Background tasks (synced from agent via events) ===
    pub bg_tasks: IndexMap<String, BgTaskDetail>,
    // === Structured tasks (synced from TaskStore via TasksChanged events) ===
    pub task_snapshots: Vec<TaskSnapshot>,
    // === Cron jobs (synced from CronScheduler via CronsChanged events) ===
    pub cron_snapshots: Vec<CronJobSnapshot>,
    // === Interaction state ===
    /// Pending sub-agent refs to be persisted (drained by caller).
    pub pending_sub_agent_refs: Vec<PendingSubAgentRef>,
    // === MCP runtime status (updated via McpStatusReport events) ===
    /// `None` = not yet received from agent; `Some` = at least one report received.
    pub mcp_status: Option<Vec<McpServerSnapshot>>,
}

/// Sub-agent reference awaiting persistence to disk.
#[derive(Debug, Clone)]
pub struct PendingSubAgentRef {
    pub name: String,
    pub session_id: String,
    pub parent: Option<String>,
    pub model: Option<String>,
}

impl SessionState {
    pub fn new(model: String, mode: String) -> Self {
        let mut agents = IndexMap::new();
        // Root agent "main" is a regular entry — no special treatment.
        let main_agent = AgentViewState {
            started_at: Some(Instant::now()),
            ..Default::default()
        };
        agents.insert(ROOT_AGENT.to_string(), main_agent);

        Self {
            agents,
            active_view: ROOT_AGENT.to_string(),
            model,
            mode,
            thinking_config: "auto".to_string(),
            root_session_id: None,
            message_feed: MessageFeed::new(200),
            bg_tasks: IndexMap::new(),
            task_snapshots: Vec::new(),
            cron_snapshots: Vec::new(),
            pending_sub_agent_refs: Vec::new(),
            mcp_status: None,
        }
    }

    /// Whether the currently viewed agent is idle (derived from observable status).
    pub fn is_active_agent_idle(&self) -> bool {
        self.agents
            .get(&self.active_view)
            .map(|a| a.is_idle())
            .unwrap_or(true)
    }

    // === Active conversation projection (zero branching) ===

    /// Conversation of the currently viewed agent.
    pub fn active_conversation(&self) -> &AgentConversation {
        &self.agents[&self.active_view].conversation
    }

    /// Mutable conversation of the currently viewed agent.
    pub fn active_conversation_mut(&mut self) -> &mut AgentConversation {
        &mut self
            .agents
            .get_mut(&self.active_view)
            .expect("active agent missing from agents map")
            .conversation
    }

    /// Conversation of a named agent.
    pub fn agent_conversation(&self, name: &str) -> Option<&AgentConversation> {
        self.agents.get(name).map(|a| &a.conversation)
    }

    /// Mutable conversation of a named agent.
    pub fn agent_conversation_mut(&mut self, name: &str) -> Option<&mut AgentConversation> {
        self.agents.get_mut(name).map(|a| &mut a.conversation)
    }
}

impl AgentViewState {
    /// Whether the agent is idle — derived solely from `observable.status`.
    ///
    /// Agent state is internal; external consumers must derive it from events,
    /// never set it directly. This replaces the former `agent_idle` flag.
    pub fn is_idle(&self) -> bool {
        matches!(
            self.observable.status,
            AgentStatus::WaitingForInput | AgentStatus::Finished | AgentStatus::Error
        )
    }

    /// Elapsed time since the agent was first observed.
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at
            .map_or(std::time::Duration::ZERO, |t| t.elapsed())
    }
}
