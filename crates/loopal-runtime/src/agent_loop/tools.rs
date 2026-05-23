use std::collections::HashSet;

use loopal_error::Result;
use loopal_provider_api::ContentBlock;
use tracing::info;

use super::runner::AgentLoopRunner;
use super::streaming_tool_exec::StreamingToolHandle;
use super::tools_phase::count_errors;
use super::turn_context::TurnContext;
use super::turn_metrics::ToolExecStats;

impl AgentLoopRunner {
    pub async fn execute_tools(
        &mut self,
        turn_ctx: &mut TurnContext,
        tool_uses: Vec<(String, String, serde_json::Value)>,
        early_handle: StreamingToolHandle,
    ) -> Result<ToolExecStats> {
        if turn_ctx.cancel.is_cancelled() {
            early_handle.discard();
            self.emit_all_interrupted(&tool_uses).await?;
            return Ok(ToolExecStats::default());
        }

        let (intercepted, remaining) = self.intercept_special_tools(turn_ctx, &tool_uses).await?;
        let intercepted_indices: HashSet<usize> = intercepted.iter().map(|(idx, _)| *idx).collect();

        let early_ids = early_handle.early_started_ids().clone();
        let non_early: Vec<(String, String, serde_json::Value)> = remaining
            .into_iter()
            .filter(|(id, _, _)| !early_ids.contains(id))
            .collect();

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

        let mut indexed_results: Vec<(usize, ContentBlock)> = Vec::new();
        indexed_results.extend(intercepted);
        indexed_results.extend(check.denied);

        let parallel = self
            .run_approved_batch(check.approved, &tool_uses, &turn_ctx.cancel)
            .await?;
        indexed_results.extend(parallel);

        let early_results = early_handle.take_results().await;
        indexed_results.extend(
            early_results
                .into_iter()
                .filter(|(idx, _)| !intercepted_indices.contains(idx)),
        );

        self.wrap_plan_results(&mut indexed_results, &intercepted_indices);
        stats.errors = count_errors(&indexed_results);

        self.finalize_tool_results(indexed_results)?;
        Ok(stats)
    }
}
