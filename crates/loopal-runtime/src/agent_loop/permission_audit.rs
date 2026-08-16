use loopal_error::{LoopalError, Result};
use loopal_protocol::{
    PermissionAuditDecision, PermissionAuditSource, PermissionDecisionAuditRequest,
    PermissionIntentRequest,
};
use loopal_tool_api::PermissionDecision;

use super::runner::AgentLoopRunner;
use crate::tool_action::PreparedToolAction;

impl AgentLoopRunner {
    pub(super) async fn audit_permission_request(
        &self,
        request: &PermissionIntentRequest,
        decision: PermissionDecision,
        source: PermissionAuditSource,
    ) -> Result<()> {
        let request = PermissionDecisionAuditRequest::from_seed(
            &request.tool_call_id,
            &request.intent_seed,
            None,
            audit_decision(decision)?,
            source,
        )
        .map_err(|error| LoopalError::Permission(error.to_string()))?;
        self.record_permission_audit(&request).await
    }

    pub(super) async fn audit_permission_decision(
        &self,
        action: &PreparedToolAction,
        decision: PermissionDecision,
        source: PermissionAuditSource,
    ) -> Result<()> {
        let request = PermissionDecisionAuditRequest::new(
            action.id(),
            action.tool_name(),
            action.action_digest(),
            action.schema_digest(),
            None,
            audit_decision(decision)?,
            source,
        )
        .map_err(|error| LoopalError::Permission(error.to_string()))?;
        self.record_permission_audit(&request).await
    }

    async fn record_permission_audit(
        &self,
        request: &PermissionDecisionAuditRequest,
    ) -> Result<()> {
        self.params
            .deps
            .protected_effect_audit
            .record_permission_decision(request)
            .await
            .map_err(|error| LoopalError::Permission(format!("permission audit failed: {error}")))
    }
}

fn audit_decision(decision: PermissionDecision) -> Result<PermissionAuditDecision> {
    match decision {
        PermissionDecision::Allow => Ok(PermissionAuditDecision::Allow),
        PermissionDecision::Deny => Ok(PermissionAuditDecision::Deny),
        PermissionDecision::Ask => Err(LoopalError::Permission(
            "unresolved permission decision cannot be audited".into(),
        )),
    }
}
