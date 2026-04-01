//! Tool precheck and permission verification phase.
//!
//! Separated from `tools.rs` to keep files under 200 lines.
//! Parallel classification and human fallback live in `tools_resolve.rs`.

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
    /// When Auto mode is active and multiple tools need classification,
    /// classifier calls are parallelized to reduce latency.
    pub(super) async fn check_tools(
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

            // Fast-path permission check (no LLM call)
            let tool_perm = self
                .params
                .deps
                .kernel
                .get_tool(name)
                .map(|t| t.permission());
            let decision = tool_perm
                .map(|p| self.params.config.permission_mode.check(p))
                .unwrap_or(PermissionDecision::Allow);

            if decision != PermissionDecision::Ask {
                approved.push((id.clone(), name.clone(), input.clone()));
                continue;
            }

            // Needs further decision — collect for batch or human
            needs_classify.push((orig_idx, id.clone(), name.clone(), input.clone()));
        }

        // Parallel auto-classification or sequential human approval
        self.resolve_pending(&mut approved, &mut denied, needs_classify)
            .await?;

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
}

pub(super) fn error_block(id: &str, content: &str) -> ContentBlock {
    ContentBlock::ToolResult {
        tool_use_id: id.to_string(),
        content: content.to_string(),
        is_error: true,
        metadata: None,
    }
}
