use loopal_message::ContentBlock;
use loopal_tool_api::{PermissionDecision, PermissionLevel};
use tracing::info;

use super::runner::AgentLoopRunner;
use super::sandbox_precheck;

pub(super) enum CheckOne {
    Denied(ContentBlock),
    Approved,
    NeedsClassify(serde_json::Value),
}

impl AgentLoopRunner {
    pub(super) async fn check_one_tool(
        &self,
        id: &str,
        name: &str,
        input: &serde_json::Value,
    ) -> loopal_error::Result<CheckOne> {
        if let Some(filter) = self.plan_tool_filter() {
            if !filter.contains(name) {
                info!(tool = name, "plan mode: tool not allowed");
                let msg = "Plan mode: this tool is not available. Use read-only tools only.";
                let block = self.emit_and_block(id, name, msg, true, None).await?;
                return Ok(CheckOne::Denied(block));
            }
            if (name == "Write" || name == "Edit") && !self.is_plan_file_target(input) {
                let plan_path = self.plan_file.path().display();
                let msg = format!("Plan mode: only the plan file ({plan_path}) can be edited.");
                let block = self.emit_and_block(id, name, msg, true, None).await?;
                return Ok(CheckOne::Denied(block));
            }
        }

        let precheck_reason = self
            .params
            .deps
            .kernel
            .get_tool(name)
            .and_then(|tool| tool.precheck(input));
        if let Some(reason) = precheck_reason {
            info!(tool = name, reason = %reason, "sandbox rejected");
            let msg = format!("Sandbox: {reason}");
            let block = self.emit_and_block(id, name, msg, true, None).await?;
            return Ok(CheckOne::Denied(block));
        }

        let extracted = sandbox_precheck::extract_paths(name, input);
        let sandbox_needs =
            sandbox_precheck::check_paths(self.tool_ctx.backend.as_ref(), &extracted);
        let tool_perm = self
            .params
            .deps
            .kernel
            .get_tool(name)
            .map(|t| t.permission());
        let effective_perm = if sandbox_needs.is_empty() {
            tool_perm
        } else {
            Some(PermissionLevel::Dangerous)
        };
        let decision = effective_perm
            .map(|p| self.params.config.permission_mode.check(p))
            .unwrap_or(PermissionDecision::Allow);
        if decision != PermissionDecision::Ask {
            if !sandbox_needs.is_empty() {
                sandbox_precheck::approve_all(self.tool_ctx.backend.as_ref(), &sandbox_needs);
            }
            return Ok(CheckOne::Approved);
        }

        let annotated = if sandbox_needs.is_empty() {
            input.clone()
        } else {
            let reasons: Vec<&str> = sandbox_needs.iter().map(|n| n.reason.as_str()).collect();
            let mut a = input.clone();
            a["sandbox_approval_reason"] = serde_json::Value::String(reasons.join("; "));
            a
        };
        Ok(CheckOne::NeedsClassify(annotated))
    }

    pub(super) fn is_plan_file_target(&self, input: &serde_json::Value) -> bool {
        let target = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        self.plan_file.matches_path(target)
    }
}
