use loopal_turn::{Turn, TurnId};

use super::TurnTracker;
use super::derive::derive_current_tool_batch_step;

impl TurnTracker {
    pub fn reopen_last_completed_turn(&mut self) -> Option<TurnId> {
        let id = self.store.reopen_last_completed_turn()?;
        self.refresh_view();
        Some(id)
    }

    pub fn rollback_reopen(&mut self, expected_id: &TurnId) {
        self.store.rollback_reopen(expected_id);
        self.current_tool_batch_step = None;
        self.refresh_view();
    }

    pub fn close_reopened_silently(&mut self, expected_id: &TurnId) {
        self.store.close_reopened_silently(expected_id);
        self.current_tool_batch_step = None;
        self.refresh_view();
    }

    // Wire-only mutation: closures MUST be idempotent on resume — no
    // TurnEvent is persisted, so fold_events reconstructs the unmutated
    // store. Examples that satisfy this: scrub_idle_tool_results,
    // condense_server_blocks. Secret redaction / audit pruning MUST take
    // the persisted-event path instead.
    pub fn with_wire_mut<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut [Turn]) -> R,
    {
        let mut clone = self.store.turns().to_vec();
        let id_snapshot: Vec<TurnId> = clone.iter().map(|t| t.id.clone()).collect();
        let result = f(&mut clone);
        debug_assert!(
            clone.len() == id_snapshot.len()
                && clone
                    .iter()
                    .zip(id_snapshot.iter())
                    .all(|(t, id)| &t.id == id),
            "with_wire_mut closure mutated turn.id values; this orphans current_turn_id"
        );
        self.store.replace_turns(clone);
        self.current_tool_batch_step = derive_current_tool_batch_step(&self.store);
        self.refresh_view();
        result
    }
}
