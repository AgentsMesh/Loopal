use loopal_message::ContentBlock;
use loopal_tool_api::PermissionDecision;
use tracing::info;

use super::runner::AgentLoopRunner;
use super::tools_inject::error_block;

impl AgentLoopRunner {
    pub(super) async fn resolve_pending(
        &self,
        approved: &mut Vec<(String, String, serde_json::Value)>,
        denied: &mut Vec<(usize, ContentBlock)>,
        pending: Vec<(usize, String, String, serde_json::Value)>,
    ) -> loopal_error::Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        self.refresh_decision_context().await;
        let frontend = &self.params.deps.frontend;
        let futs = pending.iter().map(|(_, id, name, input)| {
            let id = id.clone();
            let name = name.clone();
            let input = input.clone();
            async move { frontend.request_permission(&id, &name, &input).await }
        });
        let decisions: Vec<PermissionDecision> = futures::future::join_all(futs).await;
        for ((orig_idx, id, name, input), decision) in pending.into_iter().zip(decisions) {
            if decision == PermissionDecision::Allow {
                approved.push((id, name, input));
            } else {
                info!(tool = name.as_str(), decision = "deny", "permission");
                let msg = format!("Permission denied: tool '{name}' not allowed");
                denied.push((orig_idx, error_block(&id, &msg)));
                self.emit_tool_error(&id, &name, "Permission denied")
                    .await?;
            }
        }
        Ok(())
    }
}
