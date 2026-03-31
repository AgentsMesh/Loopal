use std::time::Instant;

use loopal_config::HookEvent;
use loopal_error::{LoopalError, Result};
use loopal_hooks::run_hook;
use loopal_kernel::Kernel;
use loopal_tool_api::{ToolContext, ToolResult, handle_overflow};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::mode::AgentMode;

const MAX_RESULT_LINES: usize = 2000;
const MAX_RESULT_BYTES: usize = 100_000;

/// Execute a tool through the full pipeline:
/// pre-hooks -> execute -> overflow-to-file -> post-hooks.
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

    // Run pre-hooks
    let pre_hooks = kernel.get_hooks(HookEvent::PreToolUse, Some(name));
    for hook_config in &pre_hooks {
        let hook_data = serde_json::json!({
            "tool_name": name,
            "tool_input": input,
        });
        match run_hook(hook_config, hook_data).await {
            Ok(result) => {
                if !result.is_success() {
                    warn!(
                        tool = name,
                        exit_code = result.exit_code,
                        "pre-hook rejected"
                    );
                    return Ok(ToolResult::error(format!(
                        "Pre-hook rejected: {}",
                        result.stderr.trim()
                    )));
                }
            }
            Err(e) => {
                warn!(tool = name, error = %e, "pre-hook failed");
                return Ok(ToolResult::error(format!("Pre-hook error: {e}")));
            }
        }
    }

    debug!(tool = name, "executing tool");
    let start = Instant::now();
    let result = tool.execute(input.clone(), ctx).await?;
    let duration = start.elapsed();
    info!(
        tool = name,
        duration_ms = duration.as_millis() as u64,
        ok = !result.is_error,
        output_len = result.content.len(),
        "tool pipeline exec"
    );

    // Overflow-to-file: save large outputs to disk, return preview + path.
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

    let post_hooks = kernel.get_hooks(HookEvent::PostToolUse, Some(name));
    for hook_config in &post_hooks {
        let hook_data = serde_json::json!({
            "tool_name": name,
            "tool_input": input,
            "tool_output": result.content,
            "is_error": result.is_error,
        });
        if let Err(e) = run_hook(hook_config, hook_data).await {
            warn!(tool = name, error = %e, "post-hook failed");
        }
    }

    Ok(result)
}
