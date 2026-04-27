use serde::{Deserialize, Serialize};

use crate::address::QualifiedAddress;
use crate::bg_task::BgTaskStatus;
use crate::cron_snapshot::CronJobSnapshot;
use crate::envelope::MessageSource;
use crate::mcp_snapshot::McpServerSnapshot;
use crate::question::Question;
use crate::task_snapshot::TaskSnapshot;

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
    /// Transient retry error — not persisted in message history.
    RetryError {
        message: String,
        attempt: u32,
        max_attempts: u32,
    },
    /// Retry succeeded or cancelled — signal retry resolution.
    RetryCleared,
    /// Agent is waiting for user input
    AwaitingInput,
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
    /// Agent transitioned into active processing (turn begins).
    /// Authoritative "started working" signal: emitted as soon as the runner
    /// moves from `WaitingForInput` to `Running`, before any LLM call or tool.
    Running,
    /// Agent loop finished
    Finished,
    /// Inter-agent message routed through MessageRouter (Observation Plane).
    MessageRouted {
        /// Origin: full `MessageSource` so observers see the kind
        /// (Human/Agent/Channel/Scheduled/System) plus any qualified address.
        source: MessageSource,
        /// Routed-to address. Carries the post-NAT view of the receiver.
        target: QualifiedAddress,
        content_preview: String,
    },
    /// Tool is requesting user to answer questions.
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
    /// Agent work was interrupted (cancel signal or new message while busy).
    Interrupted,
    /// Files modified during the completed turn.
    TurnDiffSummary { modified_files: Vec<String> },
    /// Server-side tool invoked (e.g. web_search). Observational.
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Server-side tool result received. Observational.
    ServerToolResult {
        tool_use_id: String,
        content: serde_json::Value,
    },
    /// A sub-agent was spawned by Hub.
    SubAgentSpawned {
        name: String,
        agent_id: String,
        /// Parent address (qualified when spawned cross-hub).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<QualifiedAddress>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    /// Auto-mode classifier made a permission decision.
    AutoModeDecision {
        tool_name: String,
        decision: String,
        reason: String,
        #[serde(default)]
        duration_ms: u64,
    },
    /// Session context was replaced by resuming a persisted session.
    SessionResumed {
        session_id: String,
        message_count: usize,
    },
    /// One or more `SessionResumeHook` adapters reported a non-fatal
    /// failure during a session swap. The resume itself completed
    /// (message history is on the new session) but ancillary state
    /// (cron / task list) may be stale or unloaded. Front-ends should
    /// surface this so users can investigate.
    SessionResumeWarnings {
        session_id: String,
        warnings: Vec<String>,
    },
    /// Periodic snapshot of background tasks from agent process.
    BgTaskSpawned { id: String, description: String },
    /// Incremental output from a running background task.
    BgTaskOutput { id: String, output_delta: String },
    /// Background task completed or failed (authoritative final state).
    BgTaskCompleted {
        id: String,
        status: BgTaskStatus,
        exit_code: Option<i32>,
        output: String,
    },
    /// Aggregated metrics emitted at the end of each turn.
    TurnCompleted {
        turn_id: u32,
        duration_ms: u64,
        llm_calls: u32,
        tool_calls_requested: u32,
        tool_calls_approved: u32,
        tool_calls_denied: u32,
        tool_errors: u32,
        auto_continuations: u32,
        warnings_injected: u32,
        tokens_in: u32,
        tokens_out: u32,
        modified_files: Vec<String>,
    },
    /// MCP server status snapshot (emitted on startup and on reconnect).
    McpStatusReport { servers: Vec<McpServerSnapshot> },
    /// Full task list snapshot (emitted after TaskCreate/TaskUpdate mutations).
    TasksChanged { tasks: Vec<TaskSnapshot> },
    /// Full scheduled cron jobs snapshot (emitted by the periodic bridge).
    CronsChanged { crons: Vec<CronJobSnapshot> },
}

impl AgentEventPayload {
    /// SNAT — stamp `self_hub` onto every still-local qualified address
    /// inside this payload. Already-qualified (cross-hub) addresses are
    /// left untouched. Called by the event aggregator before relaying an
    /// event upward to the MetaHub broadcast plane so receivers see a
    /// fully-qualified, self-describing payload.
    pub fn prepend_self_hub(&mut self, self_hub: &str) {
        match self {
            Self::MessageRouted { source, target, .. } => {
                source.prepend_hub_if_local(self_hub);
                target.prepend_hub_if_local(self_hub);
            }
            Self::SubAgentSpawned {
                parent: Some(p), ..
            } => {
                p.prepend_hub_if_local(self_hub);
            }
            // Other variants either carry no qualified address or carry
            // local-only data (tool ids, token counts, etc.) that are
            // meaningful only inside the originating hub.
            _ => {}
        }
    }
}
