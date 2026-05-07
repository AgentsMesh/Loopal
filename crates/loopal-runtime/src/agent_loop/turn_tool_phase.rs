use loopal_error::Result;
use tracing::info;

use super::runner::AgentLoopRunner;
use super::streaming_tool_exec::{self, StreamingToolHandle, ToolUseArrived};
use super::turn_context::TurnContext;

impl AgentLoopRunner {
    pub(super) async fn execute_tool_phase(
        &mut self,
        turn_ctx: &mut TurnContext,
        tool_uses: Vec<(String, String, serde_json::Value)>,
    ) -> Result<()> {
        self.update_fork_snapshot(&tool_uses);

        let kernel = std::sync::Arc::clone(&self.params.deps.kernel);
        let mut early_handle = StreamingToolHandle::empty();
        for (idx, (id, name, input)) in tool_uses.iter().enumerate() {
            streaming_tool_exec::feed_tool(
                &mut early_handle,
                &kernel,
                &self.tool_ctx,
                self.params.config.mode,
                &ToolUseArrived {
                    index: idx,
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                },
                self.params.deps.frontend.event_emitter(),
            );
        }

        let tool_names: Vec<&str> = tool_uses.iter().map(|(_, n, _)| n.as_str()).collect();
        info!(tool_count = tool_uses.len(), ?tool_names, "tool exec start");
        let cancel = &turn_ctx.cancel;
        turn_ctx.metrics.tool_calls_requested += tool_uses.len() as u32;
        let stats = self
            .execute_tools_with_early(tool_uses.clone(), cancel, early_handle)
            .await?;
        turn_ctx.metrics.tool_calls_approved += stats.approved;
        turn_ctx.metrics.tool_calls_denied += stats.denied;
        turn_ctx.metrics.tool_errors += stats.errors;
        info!("tool exec complete");

        // budget_limit must queue before pending_warnings drains
        self.maybe_inject_budget_limit_warning(turn_ctx).await;

        let warnings = std::mem::take(&mut turn_ctx.pending_warnings);
        self.params.store.append_warnings_to_last_user(warnings);

        self.inject_pending_messages().await;
        let result_blocks = self
            .params
            .store
            .messages()
            .last()
            .map(|m| m.content.as_slice())
            .unwrap_or(&[]);
        for obs in &mut self.observers {
            obs.on_after_tools(turn_ctx, &tool_uses, result_blocks);
        }
        Ok(())
    }
}
