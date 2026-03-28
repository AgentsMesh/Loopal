use serde::{Deserialize, Serialize};

use crate::question::Question;

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
        Self {
            agent_name: None,
            payload,
        }
    }

    /// Convenience: create a named sub-agent event.
    pub fn named(name: impl Into<String>, payload: AgentEventPayload) -> Self {
        Self {
            agent_name: Some(name.into()),
            payload,
        }
    }
}

/// Event payload. Runner/LLM/Tools only construct this enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEventPayload {
    /// Streaming text chunk from LLM
    Stream { text: String },

    /// Streaming thinking/reasoning chunk from LLM
    ThinkingStream { text: String },

    /// Thinking phase completed
    ThinkingComplete { token_count: u32 },

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
        /// Wall-clock execution time in milliseconds (filled by runtime).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        /// True when this tool signals task completion (AttemptCompletion).
        #[serde(default)]
        is_completion: bool,
        /// Structured data from the tool (e.g. bytes_written for Write).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },

    /// Periodic progress update for long-running tools (e.g. Bash).
    ToolProgress {
        id: String,
        name: String,
        /// Latest output tail or status message.
        output_tail: String,
        /// Elapsed time in milliseconds since tool started.
        elapsed_ms: u64,
    },

    /// Marks the start of a parallel tool batch (3+ tools executing concurrently).
    ToolBatchStart { tool_ids: Vec<String> },

    /// Tool requires user permission
    ToolPermissionRequest {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Error occurred
    Error { message: String },

    /// Transient retry error — displayed as an in-place banner, not in message history.
    RetryError {
        message: String,
        attempt: u32,
        max_attempts: u32,
    },

    /// Retry succeeded or cancelled — clear the banner.
    RetryCleared,

    /// Agent is waiting for user input
    AwaitingInput,

    /// Max turns reached
    MaxTurnsReached { turns: u32 },

    /// LLM output truncated by max_tokens; auto-continuing.
    AutoContinuation {
        continuation: u32,
        max_continuations: u32,
    },

    /// Token usage update
    TokenUsage {
        input_tokens: u32,
        output_tokens: u32,
        context_window: u32,
        cache_creation_input_tokens: u32,
        cache_read_input_tokens: u32,
        thinking_tokens: u32,
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

    /// Tool is requesting user to answer questions via TUI dialog.
    UserQuestionRequest {
        id: String,
        questions: Vec<Question>,
    },

    /// Conversation was rewound; remaining_turns is the count after truncation.
    Rewound { remaining_turns: usize },

    /// Conversation was compacted; old messages removed to reduce context.
    Compacted {
        kept: usize,
        removed: usize,
        tokens_before: u32,
        tokens_after: u32,
        /// "smart" (LLM summarization) or "emergency" (blind truncation).
        strategy: String,
    },

    /// Agent work was interrupted by user (ESC or new message while busy).
    Interrupted,

    /// Files modified during the completed turn.
    TurnDiffSummary { modified_files: Vec<String> },

    /// Server-side tool invoked (e.g. web_search). Informational for TUI display.
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Server-side tool result received. Informational for TUI display.
    ServerToolResult {
        tool_use_id: String,
        content: serde_json::Value,
    },

    /// A sub-agent was spawned by Hub.
    SubAgentSpawned {
        name: String,
        agent_id: String,
        /// Parent agent name (None for root-spawned agents).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
        /// Model used by the spawned agent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
    },
}
