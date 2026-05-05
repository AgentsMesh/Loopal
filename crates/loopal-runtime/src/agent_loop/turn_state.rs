use loopal_provider_api::ContinuationReason;

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
        result: LlmStreamResult,
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
