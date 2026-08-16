use loopal_protocol::PermissionAuditSource;
use loopal_tool_api::{PermissionDecision, PermissionLevel};

use super::tool_result_sink::PendingToolResult;
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use tracing::info;

use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;
use crate::tool_action::PreparedToolAction;

impl AgentLoopRunner {
    pub(super) async fn resolve_pending(
        &self,
        approved: &mut Vec<PreparedToolAction>,
        denied: &mut Vec<(usize, PendingToolResult)>,
        pending: Vec<(usize, PreparedToolAction)>,
        cancel: &TurnCancel,
    ) -> loopal_error::Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        self.refresh_decision_context().await;
        // reason: terminal UIs present one human-attention request at a time.
        for (original_index, mut action) in pending {
            let permission =
                action.permission_request(self.params.workflow_permission_causation.clone())?;
            let outcome = if cancel.is_cancelled() {
                None
            } else {
                let request = self
                    .params
                    .deps
                    .frontend
                    .request_permission_outcome(&permission);
                tokio::pin!(request);
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => None,
                    outcome = &mut request => Some(outcome),
                }
            };
            let decision = outcome.as_ref().map(|outcome| outcome.decision);
            let receipt = outcome
                .as_ref()
                .and_then(|outcome| outcome.receipt.as_ref());
            let hub_audited_allow = if decision == Some(PermissionDecision::Allow) {
                if let Some(receipt) = receipt {
                    receipt
                        .validate_for(&permission.intent_seed)
                        .map_err(|error| {
                            loopal_error::LoopalError::Permission(format!(
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
            if let Some(decision) = decision.filter(|_| !hub_audited_allow) {
                self.audit_permission_request(
                    &permission,
                    decision,
                    PermissionAuditSource::Frontend,
                )
                .await?;
            }
            if decision == Some(PermissionDecision::Allow) {
                let receipt = outcome.and_then(|outcome| outcome.receipt);
                if self.params.workflow_permission_causation.is_some()
                    && action.tool().permission() != PermissionLevel::ReadOnly
                    && receipt.is_none()
                {
                    return Err(loopal_error::LoopalError::Permission(
                        "workflow effect approval missing Hub permission receipt".into(),
                    ));
                }
                if let Some(receipt) = receipt {
                    action.set_permission_receipt(receipt);
                }
                approved.push(action);
            } else if decision.is_none() {
                let block = self
                    .pending_tool_result(
                        action.id(),
                        action.tool_name(),
                        "Interrupted by user",
                        true,
                        Some(ToolResultMetadata::cancelled(CancelCause::UserInterrupt)),
                    )
                    .await?;
                denied.push((original_index, block));
            } else {
                info!(tool = action.tool_name(), decision = "deny", "permission");
                let message = format!(
                    "Permission denied: tool '{}' not allowed",
                    action.tool_name()
                );
                let block = self
                    .pending_tool_result(action.id(), action.tool_name(), message, true, None)
                    .await?;
                denied.push((original_index, block));
            }
        }
        Ok(())
    }
}
