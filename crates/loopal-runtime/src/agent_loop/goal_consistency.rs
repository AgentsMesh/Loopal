use loopal_protocol::AgentEventPayload;
use loopal_provider_api::{ContinuationIntent, ContinuationReason};
use tracing::warn;

use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;

const CONTINUATION_SKIPPED_REASON: &str = "goal changed before continuation turn started";

impl AgentLoopRunner {
    pub(super) async fn continuation_still_consistent(&self) -> bool {
        let goal_id = match self.last_continuation_goal_id.as_ref() {
            Some(id) => id,
            None => return true,
        };
        let session = match self.params.goal_session.as_ref() {
            Some(s) => s,
            None => return false,
        };
        let goal = match session.snapshot().await {
            Ok(Some(g)) => g,
            // Transient read error: no evidence the goal changed → stay
            // consistent (don't skip) rather than drop the continuation.
            Err(_) => return true,
            // Goal genuinely gone → inconsistent.
            Ok(None) => return false,
        };
        if &goal.goal_id != goal_id {
            return false;
        }
        goal.status.participates_in_continuation()
    }

    // A goal-continuation turn whose goal changed before it started has nothing
    // to do. Recovery retries (RecoveryRetry) are not continuations and are
    // never skipped. Skipping ends the turn and returns true WITHOUT recording
    // it into turn_history — so it cannot pollute degeneration/loop counters.
    pub(super) async fn skip_stale_continuation_turn(&mut self, turn_ctx: &TurnContext) -> bool {
        if matches!(
            turn_ctx.pending_continuation,
            Some(ContinuationIntent::AutoContinue {
                reason: ContinuationReason::RecoveryRetry
            })
        ) {
            return false;
        }
        let is_continuation = self
            .turns
            .store()
            .current_turn()
            .is_some_and(|t| t.trigger.is_goal_continuation());
        if !is_continuation || self.continuation_still_consistent().await {
            return false;
        }
        warn!("{CONTINUATION_SKIPPED_REASON}");
        self.reset_continuation_state();
        self.emit_cosmetic(AgentEventPayload::ContinuationSkipped {
            reason: CONTINUATION_SKIPPED_REASON.into(),
        })
        .await;
        // Drop the stale turn by IDENTITY (its index), not position: the
        // GoalContinuation trigger would otherwise project as a dangling
        // "continue" User message into the wire context. Keying on the current
        // turn's index (vs len-1) does not assume it is the last turn. rewind is
        // event-sourced (survives resume).
        let keep = match self.turns.store().current_turn_index() {
            Some(idx) => idx,
            None => return true,
        };
        self.rewind_turns_record(keep);
        true
    }
}
