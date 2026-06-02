use loopal_protocol::AgentEventPayload;
use loopal_turn::{CancelCause, CancelledCause, TurnOutcome};

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn finalize_turn_cancellation(&mut self, cause: CancelledCause) {
        if self.turns.current_turn_id().is_none() {
            return;
        }
        self.cancel_open_tool_batch_record(cancelled_to_cancel_cause(&cause));
        self.reset_continuation_state();
        for g in &mut self.governance {
            g.on_turn_cancelled();
        }
        self.emit_cosmetic(AgentEventPayload::TurnCancelled {
            cause: cancelled_cause_wire(&cause).into(),
        })
        .await;
        self.end_turn_record(TurnOutcome::Cancelled { cause });
    }

    pub(super) fn reset_continuation_state(&mut self) {
        self.last_continuation_goal_id = None;
    }
}

fn cancelled_to_cancel_cause(c: &CancelledCause) -> CancelCause {
    match c {
        CancelledCause::UserInterrupt => CancelCause::UserInterrupt,
        CancelledCause::CrashRecovery => CancelCause::CrashRecovery,
        CancelledCause::ParentTurnAborted => CancelCause::ParentTurnAborted,
    }
}

// Explicit wire string for the TurnCancelled event, NOT the derived Debug:
// renaming a CancelledCause variant then fails to compile here instead of
// silently shifting the IPC/ACP contract.
pub(super) fn cancelled_cause_wire(c: &CancelledCause) -> &'static str {
    match c {
        CancelledCause::UserInterrupt => "UserInterrupt",
        CancelledCause::CrashRecovery => "CrashRecovery",
        CancelledCause::ParentTurnAborted => "ParentTurnAborted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_strings_are_pinned_for_every_variant() {
        let cases = [
            (CancelledCause::UserInterrupt, "UserInterrupt"),
            (CancelledCause::CrashRecovery, "CrashRecovery"),
            (CancelledCause::ParentTurnAborted, "ParentTurnAborted"),
        ];
        for (cause, wire) in cases {
            assert_eq!(cancelled_cause_wire(&cause), wire);
        }
    }
}
