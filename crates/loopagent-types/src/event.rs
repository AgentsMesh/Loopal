use serde::{Deserialize, Serialize};

/// Events emitted by the agent loop, consumed by TUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEvent {
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

    /// Token usage update
    TokenUsage {
        input_tokens: u32,
        output_tokens: u32,
        context_window: u32,
    },

    /// Mode changed
    ModeChanged { mode: String },

    /// Agent loop started
    Started,

    /// Agent loop finished
    Finished,
}
