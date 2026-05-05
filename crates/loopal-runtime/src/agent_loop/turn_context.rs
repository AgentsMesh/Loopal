//! Per-turn mutable state passed through the turn lifecycle.
//!
//! Created at the start of each turn in `run_loop`, passed to
//! `execute_turn`, and consumed at turn end. Holds data that
//! observers accumulate during a turn (e.g. file diffs, metrics).

use std::collections::BTreeSet;
use std::time::Instant;

use loopal_provider_api::ContinuationIntent;

use super::cancel::TurnCancel;
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
        }
    }
}
