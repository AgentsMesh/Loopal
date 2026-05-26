use loopal_turn::{TurnId, TurnOutcome, TurnStep};

use super::TurnStore;

impl TurnStore {
    /// Reopen the LAST turn iff it's Complete. Returns None when the store is
    /// empty, the last turn is non-Complete, or the last turn was already
    /// summarized (CompactionSummary is the latest meaningful step).
    pub(crate) fn reopen_last_completed_turn(&mut self) -> Option<TurnId> {
        if self.current_turn_id.is_some() {
            return None;
        }
        let last = self.turns.last_mut()?;
        if !matches!(last.outcome, TurnOutcome::Complete) {
            return None;
        }
        let already_summarized = last
            .body
            .steps
            .iter()
            .rev()
            .find_map(|s| match s {
                TurnStep::CompactionSummary(_) => Some(true),
                TurnStep::LlmCall { .. } | TurnStep::ToolBatch(_) => Some(false),
                _ => None,
            })
            .unwrap_or(false);
        if already_summarized {
            return None;
        }
        last.outcome = TurnOutcome::InProgress;
        let id = last.id.clone();
        self.current_turn_id = Some(id.clone());
        Some(id)
    }

    /// Cancel a reopen that produced no work — flip outcome back to Complete
    /// and clear `current_turn_id`. Mismatch panics (sibling rollback_*
    /// semantics) so split-brain state surfaces immediately.
    pub(crate) fn rollback_reopen(&mut self, expected_id: &TurnId) {
        assert!(
            self.current_turn_id.as_ref() == Some(expected_id),
            "rollback_reopen: current_turn_id mismatch (expected {expected_id:?}, got {:?})",
            self.current_turn_id
        );
        let turn = self
            .turns
            .iter_mut()
            .find(|t| &t.id == expected_id)
            .expect("rollback_reopen: turn referenced by current_turn_id missing from store");
        turn.outcome = TurnOutcome::Complete;
        self.current_turn_id = None;
    }

    /// In-memory close of a reopen+append cycle without emitting a second
    /// TurnEnded event. Replay relies on the original on-disk TurnEnded
    /// plus the new StepAppended to reach the same state.
    pub(crate) fn close_reopened_silently(&mut self, expected_id: &TurnId) {
        assert!(
            self.current_turn_id.as_ref() == Some(expected_id),
            "close_reopened_silently: current_turn_id mismatch (expected {expected_id:?}, got {:?})",
            self.current_turn_id
        );
        let turn = self
            .turns
            .iter_mut()
            .find(|t| &t.id == expected_id)
            .expect("close_reopened_silently: turn missing");
        turn.outcome = TurnOutcome::Complete;
        self.current_turn_id = None;
    }
}
