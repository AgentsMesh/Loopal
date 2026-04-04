use std::time::Instant;

use loopal_config::HookEvent;
use loopal_error::{LoopalError, Result};
use loopal_hooks::{HookContext, HookOutput, PermissionOverride};
use loopal_kernel::Kernel;
use loopal_tool_api::{ToolContext, ToolResult, handle_overflow};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::mode::AgentMode;

const MAX_RESULT_LINES: usize = 2000;
const MAX_RESULT_BYTES: usize = 100_000;

/// Execute a tool through the full pipeline:
/// pre-hooks → execute → overflow-to-file → post-hooks.
///
/// Pre-hooks can reject (PermissionOverride::Deny) or modify input (updated_input).
/// Post-hooks can inject feedback (additional_context) into the tool result.
pub async fn execute_tool(
    kernel: &Kernel,
    name: &str,
    input: Value,
    ctx: &ToolContext,
    _mode: &AgentMode,
) -> Result<ToolResult> {
    let tool = kernel
        .get_tool(name)
        .ok_or_else(|| LoopalError::Tool(loopal_error::ToolError::NotFound(name.to_string())))?;

    // ── Pre-hooks ──────────────────────────────────────────────
    let pre_outputs = kernel
        .hook_service()
        .run_hooks(
            HookEvent::PreToolUse,
            &HookContext {
                tool_name: Some(name),
                tool_input: Some(&input),
                ..Default::default()
            },
        )
        .await;

    // Check for rejections and input modifications.
    let mut effective_input = input;
    let mut input_updated = false;
    for out in &pre_outputs {
        if let Some(PermissionOverride::Deny { ref reason }) = out.permission {
            warn!(tool = name, %reason, "pre-hook rejected");
            return Ok(ToolResult::error(format!("Pre-hook rejected: {reason}")));
        }
        if let Some(ref updated) = out.updated_input {
            if input_updated {
                warn!(
                    tool = name,
                    "multiple pre-hooks modified input, later override wins"
                );
            }
            effective_input = updated.clone();
            input_updated = true;
        }
    }

    // ── Execute ────────────────────────────────────────────────
    debug!(tool = name, "executing tool");
    let start = Instant::now();
    let result = tool.execute(effective_input.clone(), ctx).await?;
    let duration = start.elapsed();
    info!(
        tool = name,
        duration_ms = duration.as_millis() as u64,
        ok = !result.is_error,
        output_len = result.content.len(),
        "tool pipeline exec"
    );

    // ── Overflow-to-file ───────────────────────────────────────
    let overflow = handle_overflow(&result.content, MAX_RESULT_LINES, MAX_RESULT_BYTES, name);
    let result = if overflow.overflowed {
        warn!(
            tool = name,
            original_bytes = result.content.len(),
            "tool result overflowed to file"
        );
        ToolResult {
            content: overflow.display,
            is_error: result.is_error,
            metadata: result.metadata,
        }
    } else {
        result
    };

    // ── Post-hooks ─────────────────────────────────────────────
    let post_outputs = kernel
        .hook_service()
        .run_hooks(
            HookEvent::PostToolUse,
            &HookContext {
                tool_name: Some(name),
                tool_input: Some(&effective_input),
                tool_output: Some(&result.content),
                is_error: Some(result.is_error),
                ..Default::default()
            },
        )
        .await;

    let result = append_post_hook_feedback(result, &post_outputs);
    Ok(result)
}

/// Collect `additional_context` from post-hook outputs and append to tool result.
fn append_post_hook_feedback(mut result: ToolResult, outputs: &[HookOutput]) -> ToolResult {
    let feedback: Vec<&str> = outputs
        .iter()
        .filter_map(|o| o.additional_context.as_deref())
        .collect();

    if feedback.is_empty() {
        return result;
    }

    result.content.push_str("\n\n[POST-HOOK FEEDBACK]\n");
    result.content.push_str(&feedback.join("\n---\n"));
    result
}
