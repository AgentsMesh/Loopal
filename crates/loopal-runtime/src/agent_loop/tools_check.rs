use loopal_provider_api::ContentBlock;
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use tracing::Instrument;

use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;
use super::sandbox_precheck;
use super::tools_check_one::CheckOne;

pub(super) struct CheckResult {
    pub approved: Vec<(String, String, serde_json::Value)>,
    pub denied: Vec<(usize, ContentBlock)>,
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
            let orig_idx = tool_uses
                .iter()
                .position(|(tid, _, _)| tid == id)
                .unwrap_or(0);

            match self.check_one_tool(id, name, input).await? {
                CheckOne::Denied(block) => denied.push((orig_idx, block)),
                CheckOne::Approved => approved.push((id.clone(), name.clone(), input.clone())),
                CheckOne::NeedsClassify(annotated) => {
                    needs_classify.push((orig_idx, id.clone(), name.clone(), annotated));
                }
            }
        }

        self.resolve_pending(&mut approved, &mut denied, needs_classify, cancel)
            .await?;

        for (_, name, input) in &approved {
            let extracted = sandbox_precheck::extract_paths(name, input);
            let needs = sandbox_precheck::check_paths(self.tool_ctx.backend.as_ref(), &extracted);
            if !needs.is_empty() {
                sandbox_precheck::approve_all(self.tool_ctx.backend.as_ref(), &needs);
            }
        }

        for (id, name, _) in &remaining[processed..] {
            let orig_idx = tool_uses
                .iter()
                .position(|(tid, _, _)| tid == id)
                .unwrap_or(0);
            let block = self
                .emit_and_block(
                    id,
                    name,
                    "Interrupted by user",
                    true,
                    Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
                )
                .await?;
            denied.push((orig_idx, block));
        }

        Ok(CheckResult { approved, denied })
    }
}
