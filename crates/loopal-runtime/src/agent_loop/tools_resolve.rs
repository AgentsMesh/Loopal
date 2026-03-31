//! Auto-mode parallel classification and human fallback for check_tools.
//!
//! Split from `tools_check.rs` to keep files under 200 lines.

use std::sync::Arc;

use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::TaskType;
use loopal_tool_api::{PermissionDecision, PermissionMode};
use tracing::info;

use super::runner::AgentLoopRunner;
use super::tools_check::error_block;

impl AgentLoopRunner {
    /// Resolve pending tools: parallel classify in Auto mode, sequential human otherwise.
    pub(super) async fn resolve_pending(
        &self,
        approved: &mut Vec<(String, String, serde_json::Value)>,
        denied: &mut Vec<(usize, ContentBlock)>,
        pending: Vec<(usize, String, String, serde_json::Value)>,
    ) -> loopal_error::Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        let is_auto = self.params.config.permission_mode == PermissionMode::Auto
            && self.params.auto_classifier.is_some();

        if is_auto {
            self.classify_parallel(approved, denied, pending).await
        } else {
            self.ask_human_sequential(approved, denied, pending).await
        }
    }

    /// Parallel LLM classification for Auto mode.
    async fn classify_parallel(
        &self,
        approved: &mut Vec<(String, String, serde_json::Value)>,
        denied: &mut Vec<(usize, ContentBlock)>,
        pending: Vec<(usize, String, String, serde_json::Value)>,
    ) -> loopal_error::Result<()> {
        let classifier = self.params.auto_classifier.as_ref().unwrap();

        // Degraded: fall back to the frontend's permission handler.
        // The consumer determines the response: TUI prompts a human,
        // headless auto-approves, sub-agents auto-deny.
        if classifier.is_degraded() {
            return self.ask_human_sequential(approved, denied, pending).await;
        }

        let context = loopal_auto_mode::prompt::build_recent_context(self.params.store.messages());
        let model = self.params.config.router.resolve(TaskType::Classification);
        let provider = match self.params.deps.kernel.resolve_provider(model) {
            Ok(p) => p,
            Err(e) => {
                // Provider failure: deny all pending rather than crash the turn.
                let msg = format!("Classifier provider error: {e}");
                return self.deny_all(denied, pending, &msg).await;
            }
        };

        // Spawn all classification tasks concurrently.
        let mut handles = Vec::with_capacity(pending.len());
        for (orig_idx, id, name, input) in &pending {
            let c = Arc::clone(classifier);
            let ctx = context.clone();
            let p = Arc::clone(&provider);
            let m = model.to_string();
            let n = name.clone();
            let inp = input.clone();
            let handle =
                tokio::spawn(async move { c.classify(&n, &inp, &ctx, p.as_ref(), &m).await });
            handles.push((*orig_idx, id.clone(), name.clone(), input.clone(), handle));
        }

        // Collect results in original order.
        for (orig_idx, id, name, input, handle) in handles {
            let result = handle
                .await
                .unwrap_or_else(|_| loopal_auto_mode::ClassifierResult {
                    decision: PermissionDecision::Deny,
                    reason: "Classifier task panicked".into(),
                    duration_ms: 0,
                });
            let _ = self
                .emit(AgentEventPayload::AutoModeDecision {
                    tool_name: name.clone(),
                    decision: match result.decision {
                        PermissionDecision::Allow => "allow",
                        PermissionDecision::Deny => "deny",
                        PermissionDecision::Ask => "ask",
                    }
                    .into(),
                    reason: result.reason.clone(),
                    duration_ms: result.duration_ms,
                })
                .await;
            if result.decision == PermissionDecision::Deny {
                info!(tool = name.as_str(), "auto-mode denied");
                let msg = format!("Auto-denied: {}", result.reason);
                denied.push((orig_idx, error_block(&id, &msg)));
                self.emit_tool_error(&id, &name, &msg).await?;
            } else {
                approved.push((id, name, input));
            }
        }
        Ok(())
    }

    /// Sequential human permission requests (Supervised mode / degraded Auto).
    async fn ask_human_sequential(
        &self,
        approved: &mut Vec<(String, String, serde_json::Value)>,
        denied: &mut Vec<(usize, ContentBlock)>,
        pending: Vec<(usize, String, String, serde_json::Value)>,
    ) -> loopal_error::Result<()> {
        for (orig_idx, id, name, input) in pending {
            let decision = self
                .params
                .deps
                .frontend
                .request_permission(&id, &name, &input)
                .await;
            if decision == PermissionDecision::Allow {
                if let Some(ref c) = self.params.auto_classifier {
                    if c.is_degraded() {
                        c.on_human_approval(&name);
                    }
                }
                approved.push((id, name, input));
            } else {
                info!(tool = name.as_str(), decision = "deny", "permission");
                let msg = format!("Permission denied: tool '{name}' not allowed");
                denied.push((orig_idx, error_block(&id, &msg)));
                self.emit_tool_error(&id, &name, "Permission denied")
                    .await?;
            }
        }
        Ok(())
    }

    /// Deny all pending tools with a reason (used for non-interactive fallback).
    async fn deny_all(
        &self,
        denied: &mut Vec<(usize, ContentBlock)>,
        pending: Vec<(usize, String, String, serde_json::Value)>,
        reason: &str,
    ) -> loopal_error::Result<()> {
        for (orig_idx, id, name, _input) in pending {
            info!(tool = name.as_str(), reason, "auto-mode denied (fallback)");
            denied.push((orig_idx, error_block(&id, reason)));
            self.emit_tool_error(&id, &name, reason).await?;
        }
        Ok(())
    }
}
