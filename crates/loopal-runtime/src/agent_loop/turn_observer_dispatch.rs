//! Stop hook and observer dispatch for the turn execution loop.
//!
//! Extracted from `turn_exec` — these are lifecycle extension points
//! with independent change reasons (hook config, observer API).

use loopal_error::Result;
use loopal_protocol::AgentEventPayload;
use tracing::warn;

use super::runner::AgentLoopRunner;
use super::turn_context::TurnContext;
use super::turn_observer::ObserverAction;

impl AgentLoopRunner {
    /// Run before-tools observers. Returns `true` if the turn should abort.
    pub(super) async fn run_before_tools(
        &mut self,
        turn_ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
    ) -> Result<bool> {
        for obs in &mut self.observers {
            match obs.on_before_tools(turn_ctx, tool_uses) {
                ObserverAction::Continue => {}
                ObserverAction::InjectWarning(msg) => {
                    turn_ctx.pending_warnings.push(msg);
                }
                ObserverAction::AbortTurn(reason) => {
                    warn!(%reason, "observer aborted turn");
                    self.emit(AgentEventPayload::Error { message: reason })
                        .await?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Run Stop lifecycle hooks. Returns feedback to inject if hooks want
    /// the agent to continue, or `None` to let the turn end.
    pub(super) async fn run_stop_hooks(&self) -> Option<String> {
        let stop_outputs = self
            .params
            .deps
            .kernel
            .hook_service()
            .run_hooks(
                loopal_config::HookEvent::Stop,
                &loopal_hooks::HookContext {
                    stop_reason: Some("end_turn"),
                    ..Default::default()
                },
            )
            .await;
        let feedback: Vec<&str> = stop_outputs
            .iter()
            .filter_map(|o| o.additional_context.as_deref())
            .collect();
        if feedback.is_empty() {
            None
        } else {
            Some(feedback.join("\n"))
        }
    }
}
