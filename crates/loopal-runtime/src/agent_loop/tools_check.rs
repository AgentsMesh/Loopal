use loopal_tool_invocation::{CancelCause, ToolResultMetadata};

use super::tool_result_sink::PendingToolResult;
use tracing::Instrument;

use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;
use super::sandbox_precheck;
use super::tools_check_one::CheckOne;
use crate::tool_action::PreparedToolAction;
use crate::tool_prepare::{ToolPreparation, prepare_tool_action};

pub(super) struct CheckResult {
    pub approved: Vec<PreparedToolAction>,
    pub denied: Vec<(usize, PendingToolResult)>,
}

impl AgentLoopRunner {
    pub(super) async fn check_tools(
        &mut self,
        remaining: &[(String, String, serde_json::Value)],
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> loopal_error::Result<CheckResult> {
        let check_span = tracing::info_span!("tool_check", tools.count = remaining.len());
        self.check_tools_inner(remaining, tool_uses, cancel)
            .instrument(check_span)
            .await
    }

    async fn check_tools_inner(
        &mut self,
        remaining: &[(String, String, serde_json::Value)],
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> loopal_error::Result<CheckResult> {
        let mut approved = Vec::new();
        let mut denied = Vec::new();
        let mut needs_classify = Vec::new();
        let mut processed = 0usize;

        for (id, name, input) in remaining {
            if cancel.is_cancelled() {
                break;
            }
            processed += 1;
            let original_index = tool_uses
                .iter()
                .position(|(tool_id, _, _)| tool_id == id)
                .unwrap_or(0);
            let prepared = match prepare_tool_action(
                &self.params.deps.kernel,
                id,
                name,
                input.clone(),
            )
            .await
            {
                Ok(ToolPreparation::Prepared(action)) => *action,
                Ok(ToolPreparation::Denied(message)) => {
                    let block = self
                        .pending_tool_result(id, name, message, true, None)
                        .await?;
                    denied.push((original_index, block));
                    continue;
                }
                Err(
                    error @ loopal_error::LoopalError::Tool(loopal_error::ToolError::NotFound(_)),
                ) => {
                    let block = self
                        .pending_tool_result(id, name, error.to_string(), true, None)
                        .await?;
                    denied.push((original_index, block));
                    continue;
                }
                Err(error) => return Err(error),
            };

            match self.check_one_tool(prepared).await? {
                CheckOne::Denied(block) => denied.push((original_index, block)),
                CheckOne::Approved(action) => approved.push(action),
                CheckOne::NeedsClassify(action) => {
                    needs_classify.push((original_index, action));
                }
            }
        }

        self.resolve_pending(&mut approved, &mut denied, needs_classify, cancel)
            .await?;

        for action in &approved {
            let extracted =
                sandbox_precheck::extract_paths(action.tool_name(), action.placeholder_input());
            let needs = sandbox_precheck::check_paths(self.tool_ctx.backend.as_ref(), &extracted);
            if !needs.is_empty() {
                sandbox_precheck::approve_all(self.tool_ctx.backend.as_ref(), &needs);
            }
        }

        for (id, name, _) in &remaining[processed..] {
            let original_index = tool_uses
                .iter()
                .position(|(tool_id, _, _)| tool_id == id)
                .unwrap_or(0);
            let block = self
                .pending_tool_result(
                    id,
                    name,
                    "Interrupted by user",
                    true,
                    Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
                )
                .await?;
            denied.push((original_index, block));
        }

        Ok(CheckResult { approved, denied })
    }
}
