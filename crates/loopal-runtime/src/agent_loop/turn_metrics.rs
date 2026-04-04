//! Per-turn metrics aggregated during the turn lifecycle.
//!
//! `TurnMetrics` is a counter-based telemetry struct accumulated
//! in `TurnContext` and emitted as `TurnCompleted` event at turn end.

/// Aggregated metrics for a single agent turn.
///
/// Counters are incremented at various execution points:
/// - `llm_calls`: each `stream_llm_with` invocation
/// - `tool_calls_*`: from `ToolExecStats` returned by `execute_tools`
/// - `auto_continuations`: from inner loop continuation count
/// - `warnings_injected`: from `pending_warnings.len()`
/// - `tokens_*`: from `TokenAccumulator` delta
#[derive(Debug, Default, Clone)]
pub struct TurnMetrics {
    /// LLM streaming calls made during this turn.
    pub llm_calls: u32,
    /// Tools requested by LLM (total across all LLM iterations in the turn).
    pub tool_calls_requested: u32,
    /// Tools that passed permission checks and were executed.
    pub tool_calls_approved: u32,
    /// Tools denied by sandbox/permission/plan-mode.
    pub tool_calls_denied: u32,
    /// Tools whose execution returned is_error=true or Err.
    pub tool_errors: u32,
    /// MaxTokens auto-continuations triggered.
    pub auto_continuations: u32,
    /// Warnings injected by observers (e.g. loop detector).
    pub warnings_injected: u32,
    /// Input tokens consumed during this turn.
    pub tokens_in: u32,
    /// Output tokens produced during this turn.
    pub tokens_out: u32,
}

/// Summary returned by `execute_tools` for metrics aggregation.
#[derive(Debug, Default)]
pub struct ToolExecStats {
    pub approved: u32,
    pub denied: u32,
    pub errors: u32,
}
