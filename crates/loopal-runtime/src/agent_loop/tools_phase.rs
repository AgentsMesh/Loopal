use std::collections::HashSet;
use std::sync::Arc;

use loopal_error::Result;
use loopal_protocol::AgentEventPayload;

use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;
use super::tool_exec::execute_approved_tools;
use super::tool_result_sink::PendingToolResult;
use crate::mode::AgentMode;
use crate::plan_file::wrap_plan_reminder;
use crate::tool_action::PreparedToolAction;

impl AgentLoopRunner {
    pub(super) async fn run_approved_batch(
        &self,
        approved: Vec<PreparedToolAction>,
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> Result<Vec<(usize, PendingToolResult)>> {
        if approved.is_empty() {
            return Ok(Vec::new());
        }
        if approved.len() >= 3 {
            let tool_ids = approved
                .iter()
                .map(|action| action.id().to_string())
                .collect();
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
        approved: Vec<PreparedToolAction>,
        tool_ids: Vec<String>,
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> Result<Vec<(usize, PendingToolResult)>> {
        self.emit_in_turn(AgentEventPayload::ToolBatchStart { tool_ids })
            .await?;
        self.execute_batch(approved, tool_uses, cancel).await
    }

    async fn execute_batch(
        &self,
        approved: Vec<PreparedToolAction>,
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> Result<Vec<(usize, PendingToolResult)>> {
        Ok(execute_approved_tools(
            approved,
            tool_uses,
            Arc::clone(&self.params.deps.kernel),
            self.tool_ctx.clone(),
            self.params.config.mode,
            &self.params.deps.frontend,
            cancel,
        )
        .await)
    }

    pub(super) fn wrap_plan_results(
        &self,
        results: &mut [(usize, PendingToolResult)],
        intercepted: &HashSet<usize>,
    ) {
        if self.params.config.mode != AgentMode::Plan {
            return;
        }
        let path = self.plan_file.path().to_string_lossy();
        for (index, result) in results {
            if !intercepted.contains(index) && !result.is_error() {
                let reminder = wrap_plan_reminder("", &path);
                result.append_content(&reminder);
            }
        }
    }
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
