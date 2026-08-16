use loopal_error::Result;
use loopal_turn::{InjectionKind, TurnStep};
use tracing::{info, warn};

use super::runner::AgentLoopRunner;
use super::streaming_tool_exec::StreamingToolHandle;
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn execute_tool_phase(
        &mut self,
        turn_ctx: &mut TurnContext,
        tool_uses: Vec<(String, String, serde_json::Value)>,
    ) -> Result<()> {
        self.update_fork_snapshot(&tool_uses);

        let early_handle = StreamingToolHandle::empty();

        let tool_names: Vec<&str> = tool_uses.iter().map(|(_, n, _)| n.as_str()).collect();
        info!(tool_count = tool_uses.len(), ?tool_names, "tool exec start");
        turn_ctx.metrics.tool_calls_requested += tool_uses.len() as u32;
        let stats = self
            .execute_tools(turn_ctx, tool_uses.clone(), early_handle)
            .await?;
        turn_ctx.metrics.tool_calls_approved += stats.approved;
        turn_ctx.metrics.tool_calls_denied += stats.denied;
        turn_ctx.metrics.tool_errors += stats.errors;
        info!("tool exec complete");

        // reason: capture result_blocks before the governance Injection step
        // append, which would shift
        // `view.messages().last()` away from the tool batch's user message
        // because Injection projects to a User text message (the warning).
        // If we captured later, DiffTracker would see Text instead of
        // ToolResult and miss every Write/Edit/Apply/MultiEdit.
        let result_blocks: Vec<_> = self
            .turns
            .view()
            .messages()
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        let warnings = std::mem::take(&mut turn_ctx.pending_warnings);
        // reason: += not = — a turn can run several tool batches, and each
        // batch drains its own warnings; overwriting would keep only the last.
        turn_ctx.metrics.warnings_injected += warnings.len() as u32;
        if !warnings.is_empty() {
            let text = warnings.join("\n");
            if let Err(e) = self.append_step_record(TurnStep::Injection {
                kind: InjectionKind::SystemNote,
                text,
            }) {
                warn!(error = %e, "append_step(Injection) failed for governance warnings");
            }
        }

        // Frontend input stays in its reliable mpsc queue until run_loop's
        // turn-boundary arbitration. Applying a message or lifecycle/history
        // control here would mutate TurnStore while the old TurnContext is
        // still scoped and mix the tool follow-up across logical turns.
        // A cancelled batch's results are synthetic "Interrupted by user" errors;
        // feeding them to LoopDetector would let repeated interrupts accrue a
        // false loop streak. Hooks (DiffTracker) still run on partial writes.
        if !turn_ctx.cancel.is_cancelled() {
            for g in &mut self.governance {
                g.on_after_tools(turn_ctx, &tool_uses, &result_blocks);
            }
        }
        for h in &mut self.hooks {
            h.on_after_tools(turn_ctx, &tool_uses, &result_blocks);
        }
        Ok(())
    }
}
