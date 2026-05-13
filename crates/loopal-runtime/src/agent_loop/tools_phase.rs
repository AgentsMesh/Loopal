use std::collections::HashSet;
use std::sync::Arc;

use loopal_error::Result;
use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;

use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;
use super::tool_exec::execute_approved_tools;
use crate::mode::AgentMode;
use crate::plan_file::wrap_plan_reminder;

impl AgentLoopRunner {
    pub(super) async fn run_approved_batch(
        &self,
        approved: Vec<(String, String, serde_json::Value)>,
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> Result<Vec<(usize, ContentBlock)>> {
        if approved.is_empty() {
            return Ok(Vec::new());
        }
        if approved.len() >= 3 {
            let tool_ids: Vec<String> = approved.iter().map(|(id, _, _)| id.clone()).collect();
            let batch_id = loopal_protocol::event_id::next_event_id();
            loopal_protocol::event_id::scope_correlation(
                batch_id,
                self.batch_with_announce(approved, tool_ids, tool_uses, cancel),
            )
            .await
        } else {
            self.execute_batch(approved, tool_uses, cancel).await
        }
    }

    async fn batch_with_announce(
        &self,
        approved: Vec<(String, String, serde_json::Value)>,
        tool_ids: Vec<String>,
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> Result<Vec<(usize, ContentBlock)>> {
        self.emit(AgentEventPayload::ToolBatchStart { tool_ids })
            .await?;
        self.execute_batch(approved, tool_uses, cancel).await
    }

    async fn execute_batch(
        &self,
        approved: Vec<(String, String, serde_json::Value)>,
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> Result<Vec<(usize, ContentBlock)>> {
        let kernel = Arc::clone(&self.params.deps.kernel);
        let tool_ctx = self.tool_ctx.clone();
        let mode = self.params.config.mode;
        Ok(execute_approved_tools(
            approved,
            tool_uses,
            kernel,
            tool_ctx,
            mode,
            &self.params.deps.frontend,
            cancel,
        )
        .await)
    }

    pub(super) fn wrap_plan_results(
        &self,
        results: &mut [(usize, ContentBlock)],
        intercepted: &HashSet<usize>,
    ) {
        if self.params.config.mode != AgentMode::Plan {
            return;
        }
        let plan_path = self.plan_file.path().to_string_lossy().to_string();
        for (idx, block) in results {
            if intercepted.contains(idx) {
                continue;
            }
            if let ContentBlock::ToolResult {
                content, is_error, ..
            } = block
                && !*is_error
            {
                *content = wrap_plan_reminder(content, &plan_path);
            }
        }
    }
}

pub(super) fn count_errors(results: &[(usize, ContentBlock)]) -> u32 {
    results
        .iter()
        .filter(|(_, b)| matches!(b, ContentBlock::ToolResult { is_error: true, .. }))
        .count() as u32
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn scope_correlation_sets_id_inside_scope() {
        loopal_protocol::event_id::scope_correlation(42, async {
            assert_eq!(loopal_protocol::event_id::current_correlation_id(), 42);
        })
        .await;
        assert_eq!(loopal_protocol::event_id::current_correlation_id(), 0);
    }
}
