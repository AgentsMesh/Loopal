use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use loopal_kernel::Kernel;
use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::ToolContext;
use tracing::{Instrument, error, info};

use crate::frontend::traits::AgentFrontend;
use crate::mode::AgentMode;
use crate::tool_pipeline::execute_tool;

use super::cancel::TurnCancel;

/// Execute approved tools in parallel via JoinSet, with cancellation support.
///
/// Each tool runs concurrently; results are collected and sorted by original index.
/// When the `cancel` scope fires (ESC / message-while-busy), remaining tasks are
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
        let tool_ctx = tool_ctx.clone();
        let emitter = frontend.event_emitter();
        let span = parent_span.clone();
        let id = id.clone();
        let name = name.clone();
        let input = input.clone();

        let original_idx = tool_uses
            .iter()
            .position(|(tid, _, _)| tid == &id)
            .unwrap_or(0);

        join_set.spawn(
            async move {
                let tool_start = Instant::now();
                let result = execute_tool(&kernel, &name, input, &tool_ctx, &mode).await;
                let tool_duration = tool_start.elapsed();

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
                        };
                        let block = ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: result.content,
                            is_error: result.is_error,
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
                        };
                        let block = ContentBlock::ToolResult {
                            tool_use_id: id,
                            content: err_msg,
                            is_error: true,
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

    // Collect results, racing against cancellation
    let mut results = Vec::new();
    let mut collected_ids: HashSet<String> = HashSet::new();

    loop {
        if cancel.is_cancelled() {
            info!("cancelled before collecting, aborting remaining tools");
            join_set.abort_all();
            break;
        }
        tokio::select! {
            biased;
            join_result = join_set.join_next() => {
                let Some(join_result) = join_result else { break; };
                match join_result {
                    Ok((idx, block)) => {
                        if let ContentBlock::ToolResult { ref tool_use_id, .. } = block {
                            collected_ids.insert(tool_use_id.clone());
                        }
                        results.push((idx, block));
                    }
                    Err(e) if e.is_cancelled() => {} // expected after abort_all
                    Err(e) => error!(error = %e, "tool task panicked"),
                }
            }
            _ = cancel.cancelled() => {
                info!("cancelled during tool execution, aborting remaining tools");
                join_set.abort_all();
                // Drain any already-completed tasks
                while let Some(join_result) = join_set.join_next().await {
                    if let Ok((idx, block)) = join_result {
                        if let ContentBlock::ToolResult { ref tool_use_id, .. } = block {
                            collected_ids.insert(tool_use_id.clone());
                        }
                        results.push((idx, block));
                    }
                }
                break;
            }
        }
    }

    // Synthesise "Interrupted by user" for tools that were not collected
    let emitter = frontend.event_emitter();
    for (id, name, _) in &approved {
        if collected_ids.contains(id) {
            continue;
        }
        let orig_idx = tool_uses
            .iter()
            .position(|(tid, _, _)| tid == id)
            .unwrap_or(0);
        let _ = emitter
            .emit(AgentEventPayload::ToolResult {
                id: id.clone(),
                name: name.clone(),
                result: "Interrupted by user".into(),
                is_error: true,
            })
            .await;
        results.push((
            orig_idx,
            ContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: "Interrupted by user".into(),
                is_error: true,
            },
        ));
    }

    results
}
