use loopal_protocol::{DegenerationSummary, MessageSource};
use loopal_provider_api::ContentBlock;

use super::super::turn_context::TurnContext;
use super::super::turn_history::{TurnHistory, TurnRecord};

#[derive(Debug)]
pub enum Verdict {
    Continue,
    InjectWarning(String),
    // reason: telemetry/frontend; feedback_to_model: written into the
    // conversation so the model reads it next turn and changes strategy.
    AbortTurn {
        reason: String,
        feedback_to_model: String,
    },
}

/// Side-effect a `Governance` requests after a completed turn. The runtime
/// reads it and applies the action; the trait stays decoupled from runner
/// internals (it cannot mutate `ContinuationGate` directly).
#[derive(Debug, Clone)]
pub enum PostTurnAction {
    None,
    /// Close the `ContinuationGate` with a deadline + emit
    /// `DegenerationDetected` for observers + inject an in-band warning
    /// the model sees on its next prompt.
    Degeneration {
        summary: DegenerationSummary,
        feedback_to_model: String,
    },
}

// Decision-making role: may veto a turn by returning AbortTurn.
// Implementations CAN mutate shared TurnContext fields (warnings, metrics).
pub trait Governance: Send + Sync {
    fn on_before_tools(
        &mut self,
        _ctx: &mut TurnContext,
        _tool_uses: &[(String, String, serde_json::Value)],
    ) -> Verdict {
        Verdict::Continue
    }

    // Self-decide whether the envelope marks a task boundary; runtime does
    // not pre-classify so each Governance can apply its own semantics.
    fn on_envelope_received(&mut self, _source: &MessageSource) {}

    // Post-execution observation for decision-makers that need tool OUTPUT to
    // decide (e.g. LoopDetector keys its signature on input+output so that a
    // tool re-reading a mutating path — same args, different result — is not
    // flagged as a loop). `results[i]` is index-matched to `tool_uses[i]`.
    // Cannot veto (the batch already ran); it feeds the next on_before_tools.
    fn on_after_tools(
        &mut self,
        _ctx: &mut TurnContext,
        _tool_uses: &[(String, String, serde_json::Value)],
        _results: &[ContentBlock],
    ) {
    }

    // Compaction just rewrote earlier history: any cross-turn state derived
    // from the pre-compact conversation (e.g. signature counters that index
    // tool calls now absent from the store) is stale and must reset.
    // Called once after the boundary marker is committed and the in-memory
    // store has advanced. Default is no-op for governances without
    // cross-turn state.
    fn on_compact_completed(&mut self) {}

    // A turn was cancelled (user interrupt / parent abort). Cross-turn state
    // accrued from its batches is not a valid sample — a user interrupt should
    // reset a loop streak rather than let it span the cancellation. Default
    // no-op for governances whose state is not turn-scoped.
    fn on_turn_cancelled(&mut self) {}

    /// Inspect the completed turn alongside trailing history; default
    /// returns `PostTurnAction::None`. Cross-turn safety nets (degeneration
    /// detector, repetition guard) override this.
    fn on_after_turn(&mut self, _record: &TurnRecord, _history: &TurnHistory) -> PostTurnAction {
        PostTurnAction::None
    }
}

// Non-decision role: CANNOT veto a turn (no Verdict return).
// Implementations MAY mutate shared TurnContext fields used by downstream
// consumers (e.g. DiffTracker writes `modified_files` which `turn_telemetry`
// reads into TurnCompleted). The ISP boundary is veto power, not mutation.
pub trait TurnHook: Send + Sync {
    fn on_turn_start(&mut self, _ctx: &mut TurnContext) {}

    // `results[i]` is the ToolResult for `tool_uses[i]` — index-matched.
    fn on_after_tools(
        &mut self,
        _ctx: &mut TurnContext,
        _tool_uses: &[(String, String, serde_json::Value)],
        _results: &[ContentBlock],
    ) {
    }

    fn on_turn_end(&mut self, _ctx: &TurnContext) {}
}
