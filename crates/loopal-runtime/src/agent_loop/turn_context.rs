//! Per-turn mutable state passed through the turn lifecycle.
//!
//! Created at the start of each turn in `run_loop`, passed to
//! `execute_turn`, and consumed at turn end. Holds data that
//! observers accumulate during a turn (e.g. file diffs, metrics).

use std::collections::BTreeSet;
use std::time::Instant;

use loopal_provider_api::ContinuationIntent;

use super::cancel::TurnCancel;
use super::token_accumulator::TokenAccumulator;
use super::turn_metrics::TurnMetrics;

/// Mutable context for a single turn (LLM → [tools → LLM]* → done).
pub struct TurnContext {
    pub turn_id: u32,
    pub cancel: TurnCancel,
    pub started_at: Instant,
    /// File paths modified during this turn (for diff tracking).
    pub modified_files: BTreeSet<String>,
    /// Warnings collected by observers (e.g. loop detector) to be appended
    /// to the tool results message. Must NOT be pushed as a separate User
    /// message — that breaks tool_use/tool_result pairing after normalization.
    pub pending_warnings: Vec<String>,
    /// Continuation intent set by the previous LLM turn (auto-continue /
    /// stop-feedback). Consumed by `prepare_chat_params_with` and translated
    /// to provider-specific protocol via `Provider::finalize_messages`.
    /// Never persisted; never enters ContextStore.
    pub pending_continuation: Option<ContinuationIntent>,
    /// Aggregated telemetry counters for this turn.
    pub metrics: TurnMetrics,
    /// Snapshot of cumulative token counters at turn start. Used to compute
    /// the per-turn delta charged against the active goal's budget. Lazily
    /// set by `execute_turn` so non-goal sessions pay no overhead.
    pub token_baseline: Option<TokenAccumulator>,
    /// Cumulative tokens already charged to the active goal during this
    /// turn. Mid-turn flushes update this; turn-end charge uses it as the
    /// low watermark to avoid double-billing.
    pub cumulative_charged_to_goal: u64,
    /// True once a `budget_limit` warning has been pushed to
    /// `pending_warnings` for the active goal in this turn. Prevents
    /// repeated injection across multiple LLM iterations within one turn.
    pub budget_limit_warning_pushed: bool,
}

impl TurnContext {
    pub fn new(turn_id: u32, cancel: TurnCancel) -> Self {
        Self {
            turn_id,
            cancel,
            started_at: Instant::now(),
            modified_files: BTreeSet::new(),
            pending_warnings: Vec::new(),
            pending_continuation: None,
            metrics: TurnMetrics::default(),
            token_baseline: None,
            cumulative_charged_to_goal: 0,
            budget_limit_warning_pushed: false,
        }
    }
}
