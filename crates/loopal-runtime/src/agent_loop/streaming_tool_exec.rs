//! Streaming tool executor — overlaps ReadOnly tool execution with LLM streaming.
//!
//! # Architecture
//!
//! `StreamingToolExec` is a standalone consumer that receives ToolUse notifications
//! via a channel. It determines whether a tool is safe to execute early (ReadOnly,
//! no side effects) and spawns execution immediately.
//!
//! The orchestration lives in `turn_exec`: it spawns both the LLM stream and this
//! executor in parallel, then merges their results. This keeps LLM streaming
//! (`llm.rs`) and tool execution (`tool_exec.rs`) as independent concerns.
//!
//! # Safety invariant
//!
//! Only `PermissionLevel::ReadOnly` tools are started early. These have no side
//! effects, so even if the stream is later truncated (MaxTokens) and the results
//! are discarded, no state is corrupted.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use loopal_kernel::Kernel;
use loopal_message::ContentBlock;
use loopal_protocol::AgentEventPayload;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolDispatch};
use tokio::task::JoinSet;
use tracing::{debug, info};

use crate::frontend::traits::EventEmitter;
use crate::mode::AgentMode;
use crate::tool_pipeline::execute_tool;

/// Notification describing a ToolUse that arrived from the LLM stream.
#[derive(Debug, Clone)]
pub struct ToolUseArrived {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// Handle returned to the orchestrator (turn_exec).
///
/// Drop or call `discard()` to cancel all early executions.
/// Call `take_results()` to await completion and collect results.
pub struct StreamingToolHandle {
    join_set: JoinSet<(usize, ContentBlock)>,
    early_ids: HashSet<String>,
}

impl StreamingToolHandle {
    /// Create an empty handle (no early tools).
    pub fn empty() -> Self {
        Self {
            join_set: JoinSet::new(),
            early_ids: HashSet::new(),
        }
    }

    /// IDs of tools that were started early — skip these in the normal pipeline.
    pub fn early_started_ids(&self) -> &HashSet<String> {
        &self.early_ids
    }

    /// Await all early-started tools and collect their results.
    pub async fn take_results(mut self) -> Vec<(usize, ContentBlock)> {
        let mut results = Vec::new();
        while let Some(Ok(item)) = self.join_set.join_next().await {
            results.push(item);
        }
        results
    }

    /// Cancel all early executions (safe: ReadOnly tools have no side effects).
    pub fn discard(mut self) {
        self.join_set.abort_all();
    }

    pub fn has_early_tools(&self) -> bool {
        !self.early_ids.is_empty()
    }
}

/// Feed a single ToolUse to the early executor. If ReadOnly, spawns immediately.
///
/// Called by turn_exec after `stream_llm_with` returns but BEFORE the normal
/// permission pipeline runs. This way ReadOnly tools start executing while
/// Supervised/Dangerous tools go through permission checks.
pub fn feed_tool(
    handle: &mut StreamingToolHandle,
    kernel: &Arc<Kernel>,
    tool_ctx: &ToolContext,
    mode: AgentMode,
    tool_use: &ToolUseArrived,
    emitter: Box<dyn EventEmitter>,
) -> bool {
    let tool = match kernel.get_tool(&tool_use.name) {
        Some(t) => t,
        None => return false,
    };

    // Skip runner-direct tools (AskUser, PlanMode, etc.) — they are handled
    // by intercept_special_tools, not the normal execution pipeline.
    if tool.dispatch() == ToolDispatch::RunnerDirect {
        return false;
    }

    if tool.permission() != PermissionLevel::ReadOnly {
        return false;
    }

    debug!(
        tool = tool_use.name,
        id = tool_use.id,
        "early-starting ReadOnly tool"
    );

    let kernel = Arc::clone(kernel);
    let tool_ctx = tool_ctx.clone();
    let id = tool_use.id.clone();
    let name = tool_use.name.clone();
    let input = tool_use.input.clone();
    let idx = tool_use.index;

    handle.join_set.spawn(async move {
        let tool_start = Instant::now();
        let result = execute_tool(&kernel, &name, input, &tool_ctx, &mode).await;
        let tool_duration = tool_start.elapsed();

        let (block, event) = match result {
            Ok(r) => {
                info!(
                    tool = name.as_str(),
                    duration_ms = tool_duration.as_millis() as u64,
                    ok = !r.is_error,
                    "tool exec (early)"
                );
                let event = AgentEventPayload::ToolResult {
                    id: id.clone(),
                    name: name.clone(),
                    result: r.content.clone(),
                    is_error: r.is_error,
                    duration_ms: Some(tool_duration.as_millis() as u64),
                    metadata: r.metadata.clone(),
                };
                let block = ContentBlock::ToolResult {
                    tool_use_id: id,
                    content: r.content,
                    is_error: r.is_error,
                    metadata: r.metadata,
                };
                (block, event)
            }
            Err(e) => {
                let err_msg = e.to_string();
                info!(
                    tool = name.as_str(),
                    duration_ms = tool_duration.as_millis() as u64,
                    ok = false, error = %err_msg,
                    "tool exec (early)"
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
                    is_error: true,
                    metadata: None,
                };
                (block, event)
            }
        };

        let _ = emitter.emit(event).await;
        (idx, block)
    });

    handle.early_ids.insert(tool_use.id.clone());
    true
}
