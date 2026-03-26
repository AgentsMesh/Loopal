//! Tool precheck and permission verification phase.
//!
//! Separated from `tools.rs` to keep files under 200 lines.

use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::PermissionDecision;
use tracing::info;

use super::cancel::TurnCancel;
use super::runner::AgentLoopRunner;

/// Result of the precheck + permission phase.
pub(super) struct CheckResult {
    pub approved: Vec<(String, String, serde_json::Value)>,
    pub denied: Vec<(usize, ContentBlock)>,
}

impl AgentLoopRunner {
    /// Phase 1: sandbox precheck + permission check for each tool.
    ///
    /// Returns approved tools and denied results (with events already emitted).
    pub(super) async fn check_tools(
        &mut self,
        remaining: &[(String, String, serde_json::Value)],
        tool_uses: &[(String, String, serde_json::Value)],
        cancel: &TurnCancel,
    ) -> loopal_error::Result<CheckResult> {
        let mut approved = Vec::new();
        let mut denied = Vec::new();
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

            // Sandbox precheck
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

            // Permission check
            let decision = self.check_permission(id, name, input).await?;
            if decision == PermissionDecision::Deny {
                info!(tool = name.as_str(), decision = "deny", "permission");
                denied.push((
                    orig_idx,
                    error_block(id, &format!("Permission denied: tool '{name}' not allowed")),
                ));
                self.emit_tool_error(id, name, "Permission denied").await?;
            } else {
                approved.push((id.clone(), name.clone(), input.clone()));
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
    async fn emit_tool_error(
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
        })
        .await
    }
}

fn error_block(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: content.to_string(),
        is_error: true,
    }
}
