use serde::{Deserialize, Serialize};

/// Lifecycle status of an agent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    #[default]
    Starting,
    Running,
    WaitingForInput,
    Finished,
    Error,
}

/// Observable agent state.
///
/// Tool-call counts (`tools_in_flight` / `tool_count` / `last_tool`) are
/// **derived from the conversation transcript**, not stored here. Consumers
/// call `AgentView::tools_in_flight()` / `tool_count()` / `last_tool()` to
/// compute them on demand. This crate (`loopal-protocol`) has no access to
/// the conversation, so the methods live on `AgentView`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservableAgentState {
    pub status: AgentStatus,
    pub turn_count: u32,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub model: String,
    #[serde(default = "default_thinking_config")]
    pub thinking_config: String,
    pub mode: String,
}

fn default_thinking_config() -> String {
    "auto".to_string()
}

impl Default for ObservableAgentState {
    fn default() -> Self {
        Self {
            status: AgentStatus::default(),
            turn_count: 0,
            input_tokens: 0,
            output_tokens: 0,
            model: String::new(),
            thinking_config: default_thinking_config(),
            mode: "act".to_string(),
        }
    }
}
