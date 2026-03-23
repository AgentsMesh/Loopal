//! Turn lifecycle extension point.
//!
//! `TurnObserver` is the open-closed mechanism for adding per-turn
//! behaviours (loop detection, diff tracking, timing, audit logging)
//! without modifying `execute_turn`.

use loopal_message::ContentBlock;

use super::turn_context::TurnContext;

/// Action returned by an observer to influence the turn flow.
#[derive(Debug)]
pub enum ObserverAction {
    /// Proceed normally.
    Continue,
    /// Inject a warning message into the conversation, then continue.
    InjectWarning(String),
    /// Abort the current turn with the given reason.
    AbortTurn(String),
}

/// Lifecycle hooks called by the runner at key points during a turn.
///
/// All methods have default no-op implementations so observers only
/// need to override the hooks they care about.
pub trait TurnObserver: Send + Sync {
    /// Called once at the start of each turn.
    fn on_turn_start(&mut self, _ctx: &mut TurnContext) {}

    /// Called after LLM returns tool calls, before execution.
    /// Return `AbortTurn` to stop execution, `InjectWarning` to
    /// append a warning message, or `Continue` to proceed.
    fn on_before_tools(
        &mut self,
        _ctx: &mut TurnContext,
        _tool_uses: &[(String, String, serde_json::Value)],
    ) -> ObserverAction {
        ObserverAction::Continue
    }

    /// Called after tool results are recorded.
    /// `tool_uses` are the LLM-requested tools; `results` are the
    /// corresponding ToolResult content blocks (matched by index).
    fn on_after_tools(
        &mut self,
        _ctx: &mut TurnContext,
        _tool_uses: &[(String, String, serde_json::Value)],
        _results: &[ContentBlock],
    ) {
    }

    /// Called when the turn ends (regardless of how it ended).
    fn on_turn_end(&mut self, _ctx: &TurnContext) {}

    /// Called when the user sends new input (reset cross-turn state).
    fn on_user_input(&mut self) {}
}
