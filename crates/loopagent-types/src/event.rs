use serde::{Deserialize, Serialize};

/// Complete event with agent identity, transported via channel to TUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    /// `None` = root agent, `Some("name")` = sub-agent.
    pub agent_name: Option<String>,
    pub payload: AgentEventPayload,
}

impl AgentEvent {
    /// Convenience: create a root-agent event.
    pub fn root(payload: AgentEventPayload) -> Self {
        Self { agent_name: None, payload }
    }

    /// Convenience: create a named sub-agent event.
    pub fn named(name: impl Into<String>, payload: AgentEventPayload) -> Self {
        Self { agent_name: Some(name.into()), payload }
    }
}

/// Event payload. Runner/LLM/Tools only construct this enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEventPayload {
    /// Streaming text chunk from LLM
    Stream { text: String },

    /// LLM is calling a tool
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Tool execution completed
    ToolResult {
        id: String,
        name: String,
        result: String,
        is_error: bool,
    },

    /// Tool requires user permission
    ToolPermissionRequest {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Error occurred
    Error { message: String },

    /// Agent is waiting for user input
    AwaitingInput,

    /// Max turns reached
    MaxTurnsReached { turns: u32 },

    /// LLM output truncated by max_tokens; auto-continuing.
    AutoContinuation { continuation: u32, max_continuations: u32 },

    /// Token usage update
    TokenUsage {
        input_tokens: u32,
        output_tokens: u32,
        context_window: u32,
        cache_creation_input_tokens: u32,
        cache_read_input_tokens: u32,
    },

    /// Mode changed
    ModeChanged { mode: String },

    /// Agent loop started
    Started,

    /// Agent loop finished
    Finished,

    /// A message was routed through the MessageRouter (Observation Plane).
    ///
    /// Emitted automatically by `MessageRouter::route()` for every envelope
    /// delivered, providing transparent inter-agent communication visibility.
    MessageRouted {
        source: String,
        target: String,
        content_preview: String,
    },
}
