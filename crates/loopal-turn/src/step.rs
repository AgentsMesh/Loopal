use serde::{Deserialize, Serialize};

use crate::content::{ServerBlock, TextBlock, ToolCall, ToolCallId, ToolResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnStep {
    LlmCall {
        model: String,
        response: AssistantOutput,
    },
    ToolBatch(OrderedToolBatch),
    /// Compaction summary + ack pair produced by smart_compact middleware.
    /// One per compaction event; carries the LLM-generated working-state.
    CompactionSummary(CompactionSummary),
    /// Post-compaction file rehydration: a tool_use/result Read pair per file
    /// injected so the model can continue without re-reading. Always follows a
    /// CompactionSummary step in the same turn (best-effort; not enforced).
    CompactionRehydrate(CompactionRehydrate),
    /// Synthetic content injected into the conversation by runtime governance,
    /// stop-feedback hooks, or config-refresh middleware. `kind` discriminates
    /// origin so downstream views (replay, audit) can label it accordingly.
    Injection {
        kind: InjectionKind,
        text: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantOutput {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub text_blocks: Vec<TextBlock>,
    // reason: Vec 顺序 = LLM stream emission 顺序; ToolBatchItem 通过 id 匹配回此处的
    // tool_calls. I4 invariant (parallel ordering) 在此处的 Vec ordering 上锁定。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    // reason: reasoning(Reasoning) 与 server tool(ToolPair) 按 LLM 流顺序混排，
    // 投影时 reasoning 必须先于其 web_search_call / 作为 Anthropic content 首块。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server_blocks: Vec<ServerBlock>,
    pub stop_reason: StopReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    Refusal,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderedToolBatch {
    // reason: Vec 顺序必须匹配 prev LlmCall.response.tool_calls 顺序; ToolBatchItem 把
    // call (含 id) 和 state 绑成一体，编译期不可能产生孤立 result.
    pub items: Vec<ToolBatchItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolBatchItem {
    pub call: ToolCall,
    pub state: ToolExecState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolExecState {
    Pending,
    Running,
    Done(ToolResult),
    Cancelled(CancelCause),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CancelCause {
    UserInterrupt,
    GovernanceAbort,
    CrashRecovery,
    Timeout,
    // Superseded: a new envelope arrived and aborted the in-progress turn.
    ParentTurnAborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionSummary {
    pub summary_text: String,
    pub ack_text: String,
    pub kept_turn_count: u32,
    pub removed_turn_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionRehydrate {
    pub files: Vec<RehydratedFile>,
    // Set when rehydrate finished with files_succeeded < files_attempted — the
    // model needs to know which files dropped out so it re-Reads them on demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RehydratedFile {
    pub path: String,
    pub tool_call_id: ToolCallId,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum InjectionKind {
    Governance,
    StopFeedback,
    ConfigRefresh,
    SystemNote,
}
