use loopal_error::Result;
use loopal_tool_api::PermissionDecision;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Check permission for a single tool call. Returns the decision.
    pub async fn check_permission(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<PermissionDecision> {
        let Some(tool) = self.params.deps.kernel.get_tool(name) else {
            return Ok(PermissionDecision::Allow);
        };

        let decision = self.params.config.permission_mode.check(tool.permission());
        if decision == PermissionDecision::Ask {
            Ok(self
                .params
                .deps
                .frontend
                .request_permission(id, name, input)
                .await)
        } else {
            Ok(decision)
        }
    }
}
