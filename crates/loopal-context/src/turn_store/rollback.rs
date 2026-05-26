use loopal_turn::TurnId;

use super::TurnStore;

impl TurnStore {
    /// Undo `start_turn`: pop trailing turn + clear `current_turn_id`. Caller
    /// passes the id from the original `start_turn` so a mismatched state
    /// panics rather than silently dropping an unrelated turn.
    pub(crate) fn rollback_last_turn(&mut self, expected_id: &TurnId) {
        assert!(
            self.current_turn_id.as_ref() == Some(expected_id),
            "rollback_last_turn: current_turn_id mismatch (expected {expected_id:?}, got {:?})",
            self.current_turn_id
        );
        assert!(
            self.turns.last().map(|t| &t.id) == Some(expected_id),
            "rollback_last_turn: last turn in vec is not the expected one"
        );
        self.turns.pop();
        self.current_turn_id = None;
    }

    /// Undo `append_step` AND restore `turn.last_step_at` to a snapshot taken
    /// before the failed append. Without restoring the timestamp, a failed
    /// persist leaves last_step_at advanced even though the step was popped,
    /// biasing microcompact's idle gauge.
    pub(crate) fn rollback_last_step_with_timestamp(
        &mut self,
        expected_turn_id: &TurnId,
        prev_last_step_at: Option<chrono::DateTime<chrono::Utc>>,
    ) {
        assert!(
            self.current_turn_id.as_ref() == Some(expected_turn_id),
            "rollback_last_step: current_turn_id mismatch (expected {expected_turn_id:?}, got {:?})",
            self.current_turn_id
        );
        let turn = self
            .turns
            .iter_mut()
            .find(|t| &t.id == expected_turn_id)
            .expect("rollback_last_step: turn referenced by current_turn_id is missing from store");
        turn.body.steps.pop();
        turn.last_step_at = prev_last_step_at;
    }
}
