use loopal_message::ContentBlock;
use loopal_protocol::MessageSource;

use super::super::turn_context::TurnContext;

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

    // Compaction just rewrote earlier history: any cross-turn state derived
    // from the pre-compact conversation (e.g. signature counters that index
    // tool calls now absent from the store) is stale and must reset.
    // Called once after the boundary marker is committed and the in-memory
    // store has advanced. Default is no-op for governances without
    // cross-turn state.
    fn on_compact_completed(&mut self) {}
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
