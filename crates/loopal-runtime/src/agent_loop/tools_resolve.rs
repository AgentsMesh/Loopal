use loopal_provider_api::ContentBlock;
use loopal_tool_api::PermissionDecision;
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use tracing::info;

use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    pub(super) async fn resolve_pending(
        &self,
        approved: &mut Vec<(String, String, serde_json::Value)>,
        denied: &mut Vec<(usize, ContentBlock)>,
        pending: Vec<(usize, String, String, serde_json::Value)>,
        cancel: &TurnCancel,
    ) -> loopal_error::Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        self.refresh_decision_context().await;
        // Human-attention requests are intentionally serialized. The Hub can
        // track multiple requests, but terminal UIs present one modal at a
        // time; emitting a whole tool batch concurrently can otherwise hide
        // all but the last request and strand the remaining RPCs.
        for (orig_idx, id, name, input) in pending {
            let decision = if cancel.is_cancelled() {
                None
            } else {
                let request = self
                    .params
                    .deps
                    .frontend
                    .request_permission(&id, &name, &input);
                tokio::pin!(request);
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => None,
                    decision = &mut request => Some(decision),
                }
            };
            if decision == Some(PermissionDecision::Allow) {
                approved.push((id, name, input));
            } else if decision.is_none() {
                let block = self
                    .emit_and_block(
                        &id,
                        &name,
                        "Interrupted by user",
                        true,
                        Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
                    )
                    .await?;
                denied.push((orig_idx, block));
            } else {
                info!(tool = name.as_str(), decision = "deny", "permission");
                let msg = format!("Permission denied: tool '{name}' not allowed");
                let block = self.emit_and_block(&id, &name, msg, true, None).await?;
                denied.push((orig_idx, block));
            }
        }
        Ok(())
    }
}
