//! Per-turn mutable state passed through the turn lifecycle.
//!
//! Created at the start of each turn in `run_loop`, passed to
//! `execute_turn`, and consumed at turn end. Holds data that
//! observers accumulate during a turn (e.g. file diffs).

use std::collections::BTreeSet;
use std::time::Instant;

use super::cancel::TurnCancel;

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
}

impl TurnContext {
    pub fn new(turn_id: u32, cancel: TurnCancel) -> Self {
        Self {
            turn_id,
            cancel,
            started_at: Instant::now(),
            modified_files: BTreeSet::new(),
            pending_warnings: Vec::new(),
        }
    }
}
