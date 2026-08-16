use loopal_provider_api::{ContinuationReason, MessageRole};

use super::llm_result::LlmStreamResult;

/// Explicit states of the inner turn loop.
///
/// `ReadyToCall` is the only state that invokes the LLM. Its precondition
/// (`store last == User || pending_continuation.is_some()`) is asserted at
/// entry — any path that records an assistant message must transition through
/// `NeedsContinuation` (sets intent) or `NeedsToolExecution → ToolResultsWritten`
/// (writes tool_result user) before returning to `ReadyToCall`.
pub(super) enum TurnState {
    ReadyToCall,
    ResponseRecorded {
        result: Box<LlmStreamResult>,
    },
    NeedsContinuation {
        reason: ContinuationReason,
    },
    NeedsToolExecution {
        tool_uses: Vec<(String, String, serde_json::Value)>,
    },
    NeedsStopFeedback {
        feedback: String,
    },
    ToolResultsWritten,
    Cancelled,
    Complete,
}

impl super::runner::AgentLoopRunner {
    pub(super) fn has_resumable_turn(&self) -> bool {
        if self.turns.current_turn_id().is_some() {
            return true;
        }
        // A User projection alone is not pending work: workflow-handled turns
        // deliberately finish without an Assistant response. Only the loader's
        // explicit crash marker makes a closed user-tailed turn resumable.
        matches!(
            self.turns.store().turns().last().map(|turn| &turn.outcome),
            Some(loopal_turn::TurnOutcome::Cancelled {
                cause: loopal_turn::CancelledCause::CrashRecovery
            })
        ) && matches!(self.turns.view().last_role(), Some(MessageRole::User))
    }
}
