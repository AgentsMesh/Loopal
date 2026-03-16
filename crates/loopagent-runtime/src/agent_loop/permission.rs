use loopagent_types::error::Result;
use loopagent_types::event::AgentEvent;
use loopagent_types::permission::PermissionDecision;
use tracing::{info, warn};

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Check permission for a single tool call. Returns the decision.
    pub(crate) async fn check_permission(
        &mut self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<PermissionDecision> {
        let Some(tool) = self.params.kernel.get_tool(name) else {
            return Ok(PermissionDecision::Allow);
        };

        let decision = self.params.permission_mode.check(tool.permission());
        if decision == PermissionDecision::Ask {
            let send_ok = self
                .params
                .event_tx
                .send(AgentEvent::ToolPermissionRequest {
                    id: id.to_string(),
                    name: name.to_string(),
                    input: input.clone(),
                })
                .await
                .is_ok();

            if !send_ok {
                warn!(tool = name, "permission channel closed, denying tool");
                return Ok(PermissionDecision::Deny);
            }

            match self.params.permission_rx.recv().await {
                Some(true) => {
                    info!(tool = name, decision = "allow", "permission");
                    Ok(PermissionDecision::Allow)
                }
                _ => {
                    info!(tool = name, decision = "deny_user", "permission");
                    Ok(PermissionDecision::Deny)
                }
            }
        } else {
            Ok(decision)
        }
    }
}
