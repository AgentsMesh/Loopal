//! Tool precheck and permission verification phase.

use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::{PermissionDecision, PermissionLevel};
use tracing::{Instrument, info};

use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;
use super::sandbox_precheck;

/// Result of the precheck + permission phase.
pub(super) struct CheckResult {
    pub approved: Vec<(String, String, serde_json::Value)>,
    pub denied: Vec<(usize, ContentBlock)>,
}

impl AgentLoopRunner {
    /// Sandbox precheck + permission check for each tool.
    pub(super) async fn check_tools(
        &mut self,
        remaining: &[(String, String, serde_json::Value)],
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> loopal_error::Result<CheckResult> {
        let check_span = tracing::info_span!("tool_check", tools.count = remaining.len());
        self.check_tools_inner(remaining, tool_uses, cancel)
            .instrument(check_span)
            .await
    }

    async fn check_tools_inner(
        &mut self,
        remaining: &[(String, String, serde_json::Value)],
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> loopal_error::Result<CheckResult> {
        let mut approved = Vec::new();
        let mut denied = Vec::new();
        let mut needs_classify = Vec::new();
        let mut processed = 0usize;

        for (id, name, input) in remaining {
            if cancel.is_cancelled() {
                break;
            }
            processed += 1;
            let orig_idx = tool_uses
                .iter()
                .position(|(tid, _, _)| tid == id)
                .unwrap_or(0);

            // Plan mode hard-enforcement: block tools not in plan_tool_filter.
            if let Some(filter) = self.plan_tool_filter() {
                if !filter.contains(name.as_str()) {
                    info!(tool = name.as_str(), "plan mode: tool not allowed");
                    denied.push((
                        orig_idx,
                        error_block(
                            id,
                            "Plan mode: this tool is not available. Use read-only tools only.",
                        ),
                    ));
                    self.emit_tool_error(id, name, "Plan mode: tool not allowed")
                        .await?;
                    continue;
                }
                // Write/Edit in plan mode: only allow plan file path.
                if (name == "Write" || name == "Edit") && !self.is_plan_file_target(input) {
                    let plan_path = self.plan_file.path().display();
                    let msg = format!("Plan mode: only the plan file ({plan_path}) can be edited.");
                    denied.push((orig_idx, error_block(id, &msg)));
                    self.emit_tool_error(id, name, &msg).await?;
                    continue;
                }
            }

            // Sandbox precheck (tool-level, e.g. Bash command checks)
            let precheck_reason = self
                .params
                .deps
                .kernel
                .get_tool(name)
                .and_then(|tool| tool.precheck(input));

            if let Some(reason) = precheck_reason {
                info!(tool = name.as_str(), reason = %reason, "sandbox rejected");
                denied.push((orig_idx, error_block(id, &format!("Sandbox: {reason}"))));
                self.emit_tool_error(id, name, &format!("Sandbox: {reason}"))
                    .await?;
                continue;
            }

            // Sandbox path pre-check: detect RequiresApproval paths before execution.
            let extracted = sandbox_precheck::extract_paths(name, input);
            let sandbox_needs =
                sandbox_precheck::check_paths(self.tool_ctx.backend.as_ref(), &extracted);

            // Determine effective permission level — elevate to Dangerous when
            // the sandbox requires approval so it flows through the permission system.
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
                // Auto-approve paths when permission mode allows it (e.g. Bypass).
                if !sandbox_needs.is_empty() {
                    sandbox_precheck::approve_all(self.tool_ctx.backend.as_ref(), &sandbox_needs);
                }
                approved.push((id.clone(), name.clone(), input.clone()));
                continue;
            }

            // Annotate input with sandbox reason for the permission prompt.
            let annotated = if sandbox_needs.is_empty() {
                input.clone()
            } else {
                let reasons: Vec<&str> = sandbox_needs.iter().map(|n| n.reason.as_str()).collect();
                let mut a = input.clone();
                a["sandbox_approval_reason"] = serde_json::Value::String(reasons.join("; "));
                a
            };

            needs_classify.push((orig_idx, id.clone(), name.clone(), annotated));
        }

        // Parallel auto-classification or sequential human approval
        self.resolve_pending(&mut approved, &mut denied, needs_classify)
            .await?;

        // Post-approval: approve sandbox paths for tools that were just granted permission.
        for (_, name, input) in &approved {
            let extracted = sandbox_precheck::extract_paths(name, input);
            let needs = sandbox_precheck::check_paths(self.tool_ctx.backend.as_ref(), &extracted);
            if !needs.is_empty() {
                sandbox_precheck::approve_all(self.tool_ctx.backend.as_ref(), &needs);
            }
        }

        // Mark unprocessed tools as interrupted
        for (id, name, _) in &remaining[processed..] {
            let orig_idx = tool_uses
                .iter()
                .position(|(tid, _, _)| tid == id)
                .unwrap_or(0);
            denied.push((orig_idx, error_block(id, "Interrupted by user")));
            self.emit_tool_error(id, name, "Interrupted by user")
                .await?;
        }

        Ok(CheckResult { approved, denied })
    }

    /// Emit a ToolResult error event (helper for denied/interrupted tools).
    pub(super) async fn emit_tool_error(
        &self,
        id: &str,
        name: &str,
        message: &str,
    ) -> loopal_error::Result<()> {
        self.emit(AgentEventPayload::ToolResult {
            id: id.to_string(),
            name: name.to_string(),
            result: message.to_string(),
            is_error: true,
            duration_ms: None,
            metadata: None,
        })
        .await
    }

    fn is_plan_file_target(&self, input: &serde_json::Value) -> bool {
        let target = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        self.plan_file.matches_path(target)
    }
}

pub(super) fn error_block(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: content.to_string(),
        is_error: true,
        metadata: None,
    }
}
