//! Single-tool permission check.
//!
//! Used for unit testing and non-batch permission queries.
//! Batch classification lives in `tools_resolve.rs`.

use loopal_error::Result;
use loopal_tool_api::{PermissionDecision, PermissionMode};

use super::runner::AgentLoopRunner;

impl AgentLoopRunner {
    /// Check permission for a single tool call (fast-path only).
    ///
    /// Returns `Allow` or `Ask` based on `PermissionMode::check()`.
    /// For `Ask`, falls back to the frontend (human approval via TUI/IPC).
    /// Auto-mode batch classification uses `resolve_pending()` instead.
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

        // Auto mode with classifier: defer to auto_classify.
        if self.params.config.permission_mode == PermissionMode::Auto {
            if let Some(ref classifier) = self.params.auto_classifier {
                let context =
                    loopal_auto_mode::prompt::build_recent_context(self.params.store.messages());
                let model = self
                    .params
                    .config
                    .router
                    .resolve(loopal_provider_api::TaskType::Classification);
                let provider = self.params.deps.kernel.resolve_provider(model)?;
                let result = classifier
                    .classify(name, input, &context, provider.as_ref(), model)
                    .await;
                return Ok(result.decision);
            }
        }

        // Fall through to human approval.
        Ok(self
            .params
            .deps
            .frontend
            .request_permission(id, name, input)
            .await)
    }
}
