use loopal_error::Result;
use loopal_tool_api::PermissionDecision;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Refresh the shared `DecisionContext` cell so any `Auto*Handler` reading
    /// it sees up-to-date conversation history. Manual handlers ignore the
    /// cell, so this is harmless when running in non-Auto mode.
    pub(super) async fn refresh_decision_context(&self) {
        let recent = loopal_auto_mode::prompt::build_recent_context(self.params.store.messages());
        self.params.deps.decision_context.set_recent(recent).await;
    }

    /// Single-tool permission check — short-circuits when the policy yields
    /// `Allow` or `Deny` directly; otherwise dispatches to the frontend.
    /// Used by integration tests; batch tools take the parallel path in
    /// `tools_resolve::resolve_pending`.
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
        if decision != PermissionDecision::Ask {
            return Ok(decision);
        }

        self.refresh_decision_context().await;
        Ok(self
            .params
            .deps
            .frontend
            .request_permission(id, name, input)
            .await)
    }
}
