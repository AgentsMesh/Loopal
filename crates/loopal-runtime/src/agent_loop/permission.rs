use loopal_error::{LoopalError, Result};
use loopal_protocol::PermissionAuditSource;
use loopal_tool_api::PermissionDecision;

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Refresh the shared `DecisionContext` cell so any `Auto*Handler` reading
    /// it sees up-to-date conversation history. Manual handlers ignore the
    /// cell, so this is harmless when running in non-Auto mode.
    pub(super) async fn refresh_decision_context(&self) {
        let recent = loopal_classifier::prompt::build_recent_context(self.turns.view().messages());
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
        let request = loopal_protocol::PermissionIntentRequest::create(
            id,
            name,
            input.clone(),
            input.clone(),
            tool.parameters_schema(),
            self.params.workflow_permission_causation.clone(),
        )
        .map_err(|error| LoopalError::Permission(error.to_string()))?;
        if decision != PermissionDecision::Ask
            && self.params.workflow_permission_causation.is_none()
        {
            self.audit_permission_request(&request, decision, PermissionAuditSource::Policy)
                .await?;
            return Ok(decision);
        }

        self.refresh_decision_context().await;
        let outcome = self
            .params
            .deps
            .frontend
            .request_permission_outcome(&request)
            .await;
        let decision = outcome.decision;
        let hub_audited_allow = if decision == PermissionDecision::Allow {
            if let Some(receipt) = outcome.receipt.as_ref() {
                receipt
                    .validate_for(&request.intent_seed)
                    .map_err(|error| {
                        LoopalError::Permission(format!(
                            "permission receipt binding mismatch: {error}"
                        ))
                    })?;
                true
            } else {
                false
            }
        } else {
            false
        };
        if !hub_audited_allow {
            self.audit_permission_request(&request, decision, PermissionAuditSource::Frontend)
                .await?;
        }
        if decision == PermissionDecision::Allow
            && self.params.workflow_permission_causation.is_some()
            && tool.permission() != loopal_tool_api::PermissionLevel::ReadOnly
            && outcome.receipt.is_none()
        {
            return Err(LoopalError::Permission(
                "workflow effect approval missing Hub permission receipt".into(),
            ));
        }
        Ok(decision)
    }
}
