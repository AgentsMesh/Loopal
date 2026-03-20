use std::sync::Arc;
use std::time::Instant;

use loopal_kernel::Kernel;
use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::ToolContext;
use tracing::{Instrument, error, info};

use crate::mode::AgentMode;
use crate::tool_pipeline::execute_tool;
use crate::frontend::traits::AgentFrontend;

/// Execute approved tools in parallel via JoinSet.
///
/// Each tool runs concurrently; results are collected and sorted by original index.
/// Events are emitted best-effort via cloned `EventEmitter`.
pub async fn execute_approved_tools(
    approved: Vec<(String, String, serde_json::Value)>,
    tool_uses: &[(String, String, serde_json::Value)],
    kernel: Arc<Kernel>,
    tool_ctx: ToolContext,
    mode: AgentMode,
    frontend: &Arc<dyn AgentFrontend>,
) -> Vec<(usize, ContentBlock)> {
    let mut join_set = tokio::task::JoinSet::new();
    let parent_span = tracing::Span::current();

    for (id, name, input) in approved {
        let kernel = Arc::clone(&kernel);
        let tool_ctx = tool_ctx.clone();
        let emitter = frontend.event_emitter();
        let span = parent_span.clone();

        let original_idx = tool_uses
            .iter()
            .position(|(tid, _, _)| tid == &id)
            .unwrap_or(0);

        join_set.spawn(async move {
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
                        id: id.clone(), name: name.clone(),
                        result: result.content.clone(), is_error: result.is_error,
                    };
                    let block = ContentBlock::ToolResult {
                        tool_use_id: id,
                        content: result.content, is_error: result.is_error,
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
                        id: id.clone(), name: name.clone(),
                        result: err_msg.clone(), is_error: true,
                    };
                    let block = ContentBlock::ToolResult {
                        tool_use_id: id, content: err_msg, is_error: true,
                    };
                    (block, event)
                }
            };

            let _ = emitter.emit(tool_result_event).await;
            (original_idx, content_block)
        }.instrument(span));
    }

    let mut results = Vec::new();
    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok((idx, block)) => results.push((idx, block)),
            Err(e) => error!(error = %e, "tool task panicked"),
        }
    }
    results
}
