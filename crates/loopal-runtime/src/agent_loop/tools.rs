use std::collections::HashSet;

use loopal_error::Result;
use tracing::info;

use super::runner::AgentLoopRunner;
use super::streaming_tool_exec::StreamingToolHandle;
use super::turn_context::TurnContext;
use super::turn_metrics::ToolExecStats;

impl AgentLoopRunner {
    pub async fn execute_tools(
        &mut self,
        turn_ctx: &mut TurnContext,
        tool_uses: Vec<(String, String, serde_json::Value)>,
        early_handle: StreamingToolHandle,
    ) -> Result<ToolExecStats> {
        if let Err(error) = self.start_tool_batch_record(&tool_uses) {
            tracing::warn!(%error, "start_tool_batch_record failed; turn log will miss this batch");
        }

        if turn_ctx.cancel.is_cancelled() {
            early_handle.discard();
            let interrupted = self.all_interrupted(&tool_uses);
            self.finalize_tool_results(interrupted).await?;
            return Ok(ToolExecStats::default());
        }

        let (intercepted, remaining) = self.intercept_special_tools(turn_ctx, &tool_uses).await?;
        let intercepted_indices: HashSet<usize> = intercepted.iter().map(|(idx, _)| *idx).collect();
        let early_ids = early_handle.early_started_ids();
        let non_early = remaining
            .into_iter()
            .filter(|(id, _, _)| !early_ids.contains(id))
            .collect::<Vec<_>>();

        info!(
            non_early = non_early.len(),
            early = early_ids.len(),
            "check_tools start"
        );
        let check = self
            .check_tools(&non_early, &tool_uses, &turn_ctx.cancel)
            .await?;
        info!(
            approved = check.approved.len(),
            denied = check.denied.len(),
            "check_tools done"
        );
        let mut stats = ToolExecStats {
            approved: check.approved.len() as u32 + early_ids.len() as u32,
            denied: check.denied.len() as u32,
            errors: 0,
        };

        let mut pending = intercepted;
        pending.extend(check.denied);
        pending.extend(
            self.run_approved_batch(check.approved, &tool_uses, &turn_ctx.cancel)
                .await?,
        );
        pending.extend(
            early_handle
                .take_results()
                .await
                .into_iter()
                .filter(|(index, _)| !intercepted_indices.contains(index)),
        );
        self.wrap_plan_results(&mut pending, &intercepted_indices);
        stats.errors = self.finalize_tool_results(pending).await?;
        Ok(stats)
    }
}
