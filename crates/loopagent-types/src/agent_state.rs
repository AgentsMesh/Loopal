use serde::{Deserialize, Serialize};

/// Lifecycle status of an agent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// Agent is initializing.
    #[default]
    Starting,
    /// Agent is actively processing (LLM call or tool execution).
    Running,
    /// Agent is idle, waiting for the next message.
    WaitingForInput,
    /// Agent has completed its task and exited normally.
    Finished,
    /// Agent terminated due to an error.
    Error,
}

/// Observable state snapshot of a single agent.
///
/// Collected on the Observation Plane and consumed by the TUI to render
/// per-agent status panels. All fields are cheap to clone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservableAgentState {
    /// Current lifecycle status.
    pub status: AgentStatus,
    /// Total number of tool calls executed.
    pub tool_count: u32,
    /// Name of the most recently invoked tool, if any.
    pub last_tool: Option<String>,
    /// Number of completed LLM turns.
    pub turn_count: u32,
    /// Cumulative input tokens consumed.
    pub input_tokens: u32,
    /// Cumulative output tokens generated.
    pub output_tokens: u32,
    /// Active model identifier (e.g. "claude-sonnet-4-20250514").
    pub model: String,
    /// Current operating mode (e.g. "act", "plan").
    pub mode: String,
}

impl Default for ObservableAgentState {
    fn default() -> Self {
        Self {
            status: AgentStatus::default(),
            tool_count: 0,
            last_tool: None,
            turn_count: 0,
            input_tokens: 0,
            output_tokens: 0,
            model: String::new(),
            mode: "act".to_string(),
        }
    }
}
