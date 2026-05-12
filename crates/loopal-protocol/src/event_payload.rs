use serde::{Deserialize, Serialize};

use crate::address::QualifiedAddress;
use crate::bg_task::BgTaskStatus;
use crate::cron_snapshot::CronJobSnapshot;
use crate::envelope::MessageSource;
use crate::mcp_snapshot::McpServerSnapshot;
use crate::question::{Question, ResolveSource};
use crate::task_snapshot::TaskSnapshot;
use crate::thread_goal::{GoalTransitionReason, ThreadGoal};

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
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },
    /// Periodic progress update for long-running tools (e.g. Bash).
    ToolProgress {
        id: String,
        name: String,
        output_tail: String,
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
    /// Emitted on `WaitingForInput` → `Running`, before any LLM/tool call.
    Running,
    /// Agent loop finished
    Finished,
    /// Inter-agent message routed through MessageRouter (Observation Plane).
    /// `target` carries the post-NAT view of the receiver.
    MessageRouted {
        source: MessageSource,
        target: QualifiedAddress,
        content_preview: String,
    },
    /// Inbox accepted a message (after `ingest_message`). Fires on the
    /// receiving runtime once the message is enqueued, for any source.
    InboxEnqueued {
        message_id: String,
        source: MessageSource,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    /// LLM consumed an inbox message — pairs with `InboxEnqueued` by id.
    InboxConsumed { message_id: String },
    UserMessageQueued {
        message_id: String,
        content: String,
        image_count: usize,
    },
    /// Tool is requesting user to answer questions.
    UserQuestionRequest {
        id: String,
        questions: Vec<Question>,
        /// True if a classifier is racing the user. UI should render a
        /// "thinking" status strip alongside the option dialog.
        #[serde(default)]
        classifier_running: bool,
    },
    /// Permission request resolved (some UI client responded). Broadcast
    /// so all other UI clients clear their local `pending_permission`
    /// dialog. `id` matches the originating `ToolPermissionRequest.id`.
    ToolPermissionResolved { id: String },
    /// Question request resolved. Broadcast so all UI clients clear their
    /// local `pending_question` dialog. `by` records who answered first.
    UserQuestionResolved {
        id: String,
        #[serde(default)]
        by: ResolveSource,
    },
    /// Auto classifier progress heartbeat. Backend ticks this every ~500ms
    /// while the classifier runs so multiple UIs see consistent elapsed time.
    ClassifierProgress { id: String, elapsed_ms: u64 },
    /// Auto classifier finished unsuccessfully. UI flips the status strip
    /// to "failed" and the user must answer manually.
    ClassifierFailed { id: String, reason: String },
    /// Auto classifier finished successfully — `UserQuestionResolved{by: Auto}`
    /// will follow shortly. Carried separately so the UI can briefly display
    /// the chosen answer before the dialog closes.
    ClassifierCompleted {
        id: String,
        answers: Vec<String>,
        duration_ms: u64,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        parent: Option<QualifiedAddress>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    PermissionDecided {
        tool_name: String,
        decision: String,
        reason: String,
        #[serde(default)]
        duration_ms: u64,
    },
    QuestionDecided {
        #[serde(default)]
        question_count: u32,
        #[serde(default)]
        duration_ms: u64,
        reason: String,
        #[serde(default)]
        source: ResolveSource,
    },
    /// Session context was replaced by resuming a persisted session.
    SessionResumed {
        session_id: String,
        message_count: usize,
    },
    /// `SessionResumeHook` adapter(s) reported non-fatal failure during a
    /// swap. Resume completed; cron/task state may be stale.
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
    /// Thread goal lifecycle event. `goal: None` means the goal was cleared.
    ThreadGoalUpdated {
        goal: Option<ThreadGoal>,
        reason: GoalTransitionReason,
    },
}
