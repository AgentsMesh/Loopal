use loopal_protocol::PermissionAuditSource;
use loopal_tool_api::{PermissionDecision, PermissionLevel};

use super::tool_result_sink::PendingToolResult;
use tracing::info;

use super::runner::AgentLoopRunner;
use super::sandbox_precheck;
use crate::tool_action::PreparedToolAction;
use crate::tool_input_validation::{validate_tool_input, validate_wire_refs};

pub(super) enum CheckOne {
    Denied(PendingToolResult),
    Approved(PreparedToolAction),
    NeedsClassify(PreparedToolAction),
}

impl AgentLoopRunner {
    pub(super) async fn check_one_tool(
        &self,
        mut action: PreparedToolAction,
    ) -> loopal_error::Result<CheckOne> {
        let id = action.id().to_string();
        let name = action.tool_name().to_string();
        let input = action.placeholder_input();
        if let Some(filter) = self.plan_tool_filter() {
            if !filter.contains(&name) {
                info!(tool = name.as_str(), "plan mode: tool not allowed");
                let message = "Plan mode: this tool is not available. Use read-only tools only.";
                let block = self
                    .pending_tool_result(&id, &name, message, true, None)
                    .await?;
                return Ok(CheckOne::Denied(block));
            }
            if (name == "Write" || name == "Edit") && !self.is_plan_file_target(input) {
                let plan_path = self.plan_file.path().display();
                let message = format!("Plan mode: only the plan file ({plan_path}) can be edited.");
                let block = self
                    .pending_tool_result(&id, &name, message, true, None)
                    .await?;
                return Ok(CheckOne::Denied(block));
            }
        }

        if let Err(reason) = validate_tool_input(action.tool().as_ref(), input) {
            return self.invalid_prepared_action(&action, reason).await;
        }
        if let Err(reason) = validate_wire_refs(input, action.tool().secret_eligible_params()) {
            return self.invalid_prepared_action(&action, reason).await;
        }
        if let Some(reason) = action.tool().precheck(input) {
            info!(tool = name.as_str(), reason = %reason, "sandbox rejected");
            let block = self
                .pending_tool_result(&id, &name, format!("Sandbox: {reason}"), true, None)
                .await?;
            return Ok(CheckOne::Denied(block));
        }

        let extracted = sandbox_precheck::extract_paths(&name, input);
        let sandbox_needs =
            sandbox_precheck::check_paths(self.tool_ctx.backend.as_ref(), &extracted);
        let effective_permission = if sandbox_needs.is_empty() {
            action.tool().permission()
        } else {
            PermissionLevel::Dangerous
        };
        let decision = self
            .params
            .config
            .permission_mode
            .check(effective_permission);
        if decision == PermissionDecision::Allow
            && (self.params.workflow_permission_causation.is_none()
                || action.tool().permission() == PermissionLevel::ReadOnly)
        {
            self.audit_permission_decision(&action, decision, PermissionAuditSource::Policy)
                .await?;
            if !sandbox_needs.is_empty() {
                sandbox_precheck::approve_all(self.tool_ctx.backend.as_ref(), &sandbox_needs);
            }
            return Ok(CheckOne::Approved(action));
        }

        if !sandbox_needs.is_empty() {
            let reasons = sandbox_needs
                .iter()
                .map(|need| need.reason.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            if let Err(error) = action.annotate_permission(reasons) {
                return self
                    .invalid_prepared_action(&action, error.to_string())
                    .await;
            }
        }
        Ok(CheckOne::NeedsClassify(action))
    }

    async fn invalid_prepared_action(
        &self,
        action: &PreparedToolAction,
        reason: String,
    ) -> loopal_error::Result<CheckOne> {
        info!(
            tool = action.tool_name(),
            rewritten = action.was_rewritten(),
            reason = %reason,
            "tool input rejected"
        );
        let message = if action.was_rewritten() {
            format!("Pre-hook produced invalid tool input: {reason}")
        } else {
            format!("Invalid tool input: {reason}")
        };
        let block = self
            .pending_tool_result(action.id(), action.tool_name(), message, true, None)
            .await?;
        Ok(CheckOne::Denied(block))
    }

    pub(super) fn is_plan_file_target(&self, input: &serde_json::Value) -> bool {
        let target = input
            .get("file_path")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        self.plan_file.matches_path(target)
    }
}
