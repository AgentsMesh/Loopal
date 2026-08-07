//! Tracks which files were modified during a turn.
//!
//! Prefers structured modified-file metadata emitted by tools. Legacy successful
//! tools fall back to extracting paths from their inputs. Emits a
//! `TurnDiffSummary` event at turn end.

use std::sync::Arc;

use loopal_edit_core::patch_parser::parse_patch;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_tool_invocation::ToolResultMetadata;

use super::governance::traits::TurnHook;
use super::turn_context::TurnContext;
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

impl TurnHook for DiffTracker {
    fn on_after_tools(
        &mut self,
        ctx: &mut TurnContext,
        tool_uses: &[(String, String, serde_json::Value)],
        results: &[ContentBlock],
    ) {
        let results_by_id: std::collections::HashMap<&str, (bool, Option<&ToolResultMetadata>)> =
            results
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::ToolResult {
                        tool_use_id,
                        is_error,
                        metadata,
                        ..
                    } => Some((tool_use_id.as_str(), (*is_error, metadata.as_ref()))),
                    _ => None,
                })
                .collect();

        for (id, name, input) in tool_uses {
            if !WRITE_TOOLS.contains(&name.as_str()) {
                continue;
            }
            let Some((is_error, metadata)) = results_by_id.get(id.as_str()).copied() else {
                continue;
            };

            // Authoritative tool-reported side effects win even for a partial
            // failure: is_error describes the overall batch, not whether none
            // of its earlier operations reached disk.
            if let Some(ToolResultMetadata::ModifiedFiles { paths }) = metadata {
                ctx.modified_files.extend(paths.iter().cloned());
                continue;
            }

            if is_error {
                continue;
            }

            // Compatibility for turns produced before ModifiedFiles metadata.
            if name == "ApplyPatch" {
                if let Some(patch) = input.get("patch").and_then(|value| value.as_str())
                    && let Ok(ops) = parse_patch(patch)
                {
                    ctx.modified_files.extend(
                        ops.into_iter()
                            .map(|op| op.path().to_string_lossy().into_owned()),
                    );
                }
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
        tracing::info!(
            files = ?files,
            count = files.len(),
            "turn modified files"
        );
        // Fire-and-forget via try_emit: spawns its own task internally,
        // avoiding writer-mutex contention with the subsequent AwaitingInput emit.
        self.frontend.try_emit(AgentEventPayload::TurnDiffSummary {
            modified_files: files,
        });
    }
}
