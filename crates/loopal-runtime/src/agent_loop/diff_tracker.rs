//! Tracks which files were modified during a turn.
//!
//! Extracts file paths from Write/Edit/MultiEdit/ApplyPatch tool inputs,
//! filtered to only those that executed successfully (is_error=false).
//! Emits a `TurnDiffSummary` event at turn end.

use std::sync::Arc;

use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;

use super::turn_context::TurnContext;
use super::turn_observer::TurnObserver;
use crate::frontend::traits::AgentFrontend;

/// Tools that are known to modify files.
const WRITE_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "ApplyPatch", "NotebookEdit"];

/// Observes tool calls and records which files were modified.
pub struct DiffTracker {
    frontend: Arc<dyn AgentFrontend>,
}

impl DiffTracker {
    pub fn new(frontend: Arc<dyn AgentFrontend>) -> Self {
        Self { frontend }
    }
}

impl TurnObserver for DiffTracker {
    fn on_after_tools(
        &mut self,
        ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
        results: &[ContentBlock],
    ) {
        // Build a set of tool_use_ids that succeeded (is_error=false)
        let succeeded: std::collections::HashSet<&str> = results
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id,
                    is_error: false,
                    ..
                } => Some(tool_use_id.as_str()),
                _ => None,
            })
            .collect();

        for (id, name, input) in tool_uses {
            if !WRITE_TOOLS.contains(&name.as_str()) {
                continue;
            }
            if !succeeded.contains(id.as_str()) {
                continue;
            }

            if let Some(path) = input
                .get("file_path")
                .or_else(|| input.get("path"))
                .or_else(|| input.get("notebook_path"))
                .and_then(|v| v.as_str())
            {
                ctx.modified_files.insert(path.to_string());
            }
            // MultiEdit has an array of edits
            if let Some(edits) = input.get("edits").and_then(|v| v.as_array()) {
                for edit in edits {
                    if let Some(p) = edit.get("file_path").and_then(|v| v.as_str()) {
                        ctx.modified_files.insert(p.to_string());
                    }
                }
            }
        }
    }

    fn on_turn_end(&mut self, ctx: &TurnContext) {
        if ctx.modified_files.is_empty() {
            return;
        }
        let files: Vec<String> = ctx.modified_files.iter().cloned().collect();
        let frontend = self.frontend.clone();
        tracing::info!(
            files = ?files,
            count = files.len(),
            "turn modified files"
        );
        // Fire-and-forget: on_turn_end is sync, emit is async
        tokio::spawn(async move {
            let _ = frontend
                .emit(AgentEventPayload::TurnDiffSummary {
                    modified_files: files,
                })
                .await;
        });
    }
}
