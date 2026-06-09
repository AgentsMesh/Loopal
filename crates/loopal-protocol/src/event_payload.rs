use serde::{Deserialize, Serialize};

use crate::address::QualifiedAddress;
use crate::bg_task::BgTaskStatus;
use crate::cron_snapshot::CronJobSnapshot;
use crate::envelope::MessageSource;
use crate::event_summary::{
    CompactPhase, CompactionSummary, ContinuationGateSummary, DegenerationSummary, SubAgentSpawn,
    TurnSummary,
};
use crate::mcp_snapshot::McpServerSnapshot;
use crate::question::{Question, ResolveSource};
use crate::task_snapshot::TaskSnapshot;
use crate::thread_goal::{GoalTransitionReason, ThreadGoal};

/// Event payload. Runner/LLM/Tools only construct this enum.
/// `#[rustfmt::skip]` keeps single-field variants on one line (200-line budget).
#[rustfmt::skip]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentEventPayload {
    Stream { text: String },
    ThinkingStream { text: String },
    ThinkingComplete { token_count: u32 },
    ToolCall {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        id: String,
        name: String,
        result: String,
        is_error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<loopal_tool_invocation::ToolResultMetadata>,
    },
    /// Periodic progress update for long-running tools (e.g. Bash).
    ToolProgress {
        id: String,
        name: String,
        output_tail: String,
        elapsed_ms: u64,
    },
    ToolBatchStart { tool_ids: Vec<String> },
    ToolPermissionRequest {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Error { message: String },
    /// Transient retry error — not persisted in message history.
    RetryError {
        message: String,
        attempt: u32,
        max_attempts: u32,
    },
    RetryCleared,
    AwaitingInput,
    AutoContinuation {
        continuation: u32,
        max_continuations: u32,
    },
    TokenUsage {
        input_tokens: u32,
        output_tokens: u32,
        context_window: u32,
        cache_creation_input_tokens: u32,
        cache_read_input_tokens: u32,
        thinking_tokens: u32,
    },
    ModeChanged { mode: String },
    ModelChanged { model: String },
    ThinkingChanged { thinking_config: String },
    PermissionModeChanged { mode: String },
    DecisionModeChanged { mode: String },
    SandboxPolicyChanged { policy: String },
    /// `context_window` syncs the budget indicator without a paired `TokenUsage`.
    Cleared { context_window: u32 },
    Started,
    /// Emitted on `WaitingForInput` → `Running`, before any LLM/tool call.
    Running,
    Finished,
    /// `target` carries the post-NAT view of the receiver.
    MessageRouted {
        source: MessageSource,
        target: QualifiedAddress,
        content_preview: String,
    },
    /// Fires on the receiving runtime once the message is enqueued, for any source.
    InboxEnqueued {
        envelope_id: String,
        source: MessageSource,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        summary: Option<String>,
    },
    /// Pairs with `InboxEnqueued` by id.
    InboxConsumed { envelope_id: String },
    UserMessageQueued {
        envelope_id: String,
        content: String,
        image_count: usize,
    },
    UserQuestionRequest {
        id: String,
        questions: Vec<Question>,
        /// True if a classifier is racing the user. UI should render a
        /// "thinking" status strip alongside the option dialog.
        #[serde(default)]
        classifier_running: bool,
    },
    /// Broadcast so other UI clients drop their `pending_permission` dialog.
    ToolPermissionResolved { id: String },
    /// Broadcast so other UI clients drop their `pending_question` dialog.
    /// `by` records who answered first.
    UserQuestionResolved {
        id: String,
        #[serde(default)]
        by: ResolveSource,
    },
    /// Auto classifier progress heartbeat (~500ms tick) so multiple UIs see consistent elapsed time.
    ClassifierProgress { id: String, elapsed_ms: u64 },
    /// Auto classifier finished unsuccessfully; UI flips strip to "failed" and the user answers manually.
    ClassifierFailed { id: String, reason: String },
    /// Auto classifier finished successfully — `UserQuestionResolved{by: Auto}` will follow shortly.
    ClassifierCompleted {
        id: String,
        answers: Vec<String>,
        duration_ms: u64,
    },
    /// `remaining_turns` is the count after truncation.
    Rewound { remaining_turns: usize },
    Compacted(CompactionSummary),
    /// Incremental compaction phase notification.
    CompactProgress { phase: CompactPhase, detail: Option<String> },
    /// Cancel signal or new message arrived while runner was busy.
    Interrupted,
    TurnDiffSummary { modified_files: Vec<String> },
    /// Observational — server-side tool invoked (e.g. web_search).
    ServerToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Observational — server-side tool result received.
    ServerToolResult {
        tool_use_id: String,
        content: serde_json::Value,
    },
    SubAgentSpawned(SubAgentSpawn),
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
    SessionResumed { session_id: String, message_count: usize },
    /// `SessionResumeHook` adapter failed during swap. Resume completed; cron/task state may be stale.
    SessionResumeWarnings { session_id: String, warnings: Vec<String> },
    BgTaskSpawned { id: String, description: String, created_at_unix_ms: u64 },
    BgTaskOutput { id: String, output_delta: String },
    /// Authoritative final state.
    BgTaskCompleted {
        id: String,
        status: BgTaskStatus,
        exit_code: Option<i32>,
        output: String,
    },
    TurnCompleted(TurnSummary),
    /// Emitted on startup and on reconnect.
    McpStatusReport { servers: Vec<McpServerSnapshot> },
    /// Emitted after TaskCreate/TaskUpdate mutations.
    TasksChanged { tasks: Vec<TaskSnapshot> },
    /// Emitted by the periodic bridge.
    CronsChanged { crons: Vec<CronJobSnapshot> },
    /// `goal: None` means the goal was cleared.
    ThreadGoalUpdated { goal: Option<ThreadGoal>, reason: GoalTransitionReason },
    HubDegraded { since_unix_ms: u64 },
    HubRecovered { duration_ms: u64 },
    /// Runtime detected a degenerate streak. Carries the auto-reopen deadline.
    DegenerationDetected(DegenerationSummary),
    /// Continuation gate opened or closed. Drives UI status indicators.
    ContinuationGateChanged(ContinuationGateSummary),
    /// A goal-continuation turn was skipped (goal changed) — keeps the skip observable, not silent.
    ContinuationSkipped { reason: String },
    /// A turn was cancelled (parent abort / governance / interrupt). `cause` is the rendered CancelledCause.
    TurnCancelled { cause: String },
}
