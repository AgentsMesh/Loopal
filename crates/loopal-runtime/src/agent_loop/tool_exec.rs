use std::sync::Arc;
use std::time::Instant;

use loopal_kernel::Kernel;
use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::{OutputTail, ToolContext};
use tracing::{Instrument, info};

use crate::frontend::traits::AgentFrontend;
use crate::mode::AgentMode;
use crate::tool_pipeline::execute_tool;

use super::cancel::TurnCancel;
use super::tool_collect::collect_results;
use super::tool_progress::maybe_spawn_progress;

/// Execute approved tools in parallel via JoinSet, with cancellation support.
///
/// Each tool runs concurrently; results are collected and sorted by original index.
/// When the `cancel` scope fires (interrupt signal), remaining tasks are
/// aborted and synthesised "Interrupted by user" results are returned.
pub async fn execute_approved_tools(
    approved: Vec<(String, String, serde_json::Value)>,
    tool_uses: &[(String, String, serde_json::Value)],
    kernel: Arc<Kernel>,
    tool_ctx: ToolContext,
    mode: AgentMode,
    frontend: &Arc<dyn AgentFrontend>,
    cancel: &TurnCancel,
) -> Vec<(usize, ContentBlock)> {
    let mut join_set = tokio::task::JoinSet::new();
    let parent_span = tracing::Span::current();

    for (id, name, input) in &approved {
        let kernel = Arc::clone(&kernel);
        let mut tool_ctx = tool_ctx.clone();
        let emitter = frontend.event_emitter();
        let progress_emitter = frontend.event_emitter();
        let span = parent_span.clone();
        let id = id.clone();
        let name = name.clone();
        let input = input.clone();

        let original_idx = tool_uses
            .iter()
            .position(|(tid, _, _)| tid == &id)
            .unwrap_or(0);

        // For Bash: create OutputTail for streaming progress
        let tail: Option<Arc<OutputTail>> = if name == "Bash" {
            let t = Arc::new(OutputTail::new(5));
            tool_ctx.output_tail = Some(Arc::clone(&t));
            Some(t)
        } else {
            None
        };

        join_set.spawn(
            async move {
                // Start progress reporter for long-running tools
                let progress =
                    maybe_spawn_progress(&name, &input, id.clone(), progress_emitter, tail);

                let tool_start = Instant::now();
                let result = execute_tool(&kernel, &name, input, &tool_ctx, &mode).await;
                let tool_duration = tool_start.elapsed();

                // Stop progress reporter
                if let Some(h) = progress {
                    h.abort();
                }

                let (content_block, tool_result_event) = match result {
                    Ok(result) => {
                        info!(
                            tool = name.as_str(),
                            duration_ms = tool_duration.as_millis() as u64,
                            ok = !result.is_error,
                            output_len = result.content.len(),
                            "tool exec (parallel)"
                        );
                        let event = AgentEventPayload::ToolResult {
                            id: id.clone(),
                            name: name.clone(),
                            result: result.content.clone(),
                            is_error: result.is_error,
                            duration_ms: Some(tool_duration.as_millis() as u64),
                            is_completion: result.is_completion,
                            metadata: result.metadata.clone(),
                        };
                        let block = ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: result.content,
                            is_error: result.is_error,
                            is_completion: result.is_completion,
                            metadata: result.metadata,
                        };
                        (block, event)
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        info!(
                            tool = name.as_str(),
                            duration_ms = tool_duration.as_millis() as u64,
                            ok = false, error = %err_msg,
                            "tool exec (parallel)"
                        );
                        let event = AgentEventPayload::ToolResult {
                            id: id.clone(),
                            name: name.clone(),
                            result: err_msg.clone(),
                            is_error: true,
                            duration_ms: Some(tool_duration.as_millis() as u64),
                            is_completion: false,
                            metadata: None,
                        };
                        let block = ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: err_msg,
                            is_error: true,
                            is_completion: false,
                            metadata: None,
                        };
                        (block, event)
                    }
                };

                let _ = emitter.emit(tool_result_event).await;
                (original_idx, content_block)
            }
            .instrument(span),
        );
    }

    collect_results(&mut join_set, &approved, tool_uses, frontend, cancel).await
}
