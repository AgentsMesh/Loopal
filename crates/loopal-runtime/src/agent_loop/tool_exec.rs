use std::sync::Arc;
use std::time::Instant;

use loopal_kernel::Kernel;
use loopal_protocol::AgentEventPayload;
use loopal_provider_api::ContentBlock;
use loopal_tool_api::{OutputTail, ToolContext, ToolResult};
use serde_json::Value;
use tracing::{Instrument, info};

use crate::frontend::traits::AgentFrontend;
use crate::mode::AgentMode;
use crate::tool_pipeline::execute_tool;

use super::cancel::TurnCancel;
use super::tool_collect::collect_results;
use super::tool_progress::maybe_spawn_progress;

/// Single convergence point for tool execution: applies the per-tool watchdog
/// deadline. Both the normal pipeline and the streaming early-start path
/// (`feed_tool`) route through here, so a tool can never run unbounded on one
/// path while bounded on the other.
pub(crate) async fn execute_tool_watchdogged(
    kernel: &Kernel,
    name: &str,
    input: Value,
    ctx: &ToolContext,
    mode: &AgentMode,
) -> loopal_error::Result<ToolResult> {
    let Some(deadline) = super::tool_watchdog::watchdog_deadline(name, &input) else {
        return execute_tool(kernel, name, input, ctx, mode).await;
    };
    match tokio::time::timeout(deadline, execute_tool(kernel, name, input, ctx, mode)).await {
        Ok(r) => r,
        Err(_) => Ok(super::tool_watchdog::timeout_result(deadline)),
    }
}

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
            loopal_protocol::event_id::propagate_to_spawn(async move {
                let progress =
                    maybe_spawn_progress(&name, &input, id.clone(), progress_emitter, tail);

                let tool_start = Instant::now();
                let result =
                    execute_tool_watchdogged(&kernel, &name, input, &tool_ctx, &mode).await;
                let tool_duration = tool_start.elapsed();

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
                            metadata: result.metadata.clone(),
                        };
                        let mut images = result.images;
                        if !images.is_empty()
                            && let Some(store) = crate::hydrate::resource_store()
                        {
                            crate::hydrate::maybe_persist_inline_images(
                                store.as_ref(),
                                &tool_ctx.session_id,
                                &mut images,
                                kernel.settings().images.inline_threshold_bytes,
                            )
                            .await;
                        }
                        let block = ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: result.content,
                            images,
                            is_error: result.is_error,
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
                            metadata: None,
                        };
                        let block = ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: err_msg,
                            images: Vec::new(),
                            is_error: true,
                            metadata: None,
                        };
                        (block, event)
                    }
                };

                emitter
                    .emit_best_effort(tool_result_event, "agent_loop::tool_exec::tool_result")
                    .await;
                (original_idx, content_block)
            })
            .instrument(span),
        );
    }

    collect_results(&mut join_set, &approved, tool_uses, frontend, cancel).await
}
