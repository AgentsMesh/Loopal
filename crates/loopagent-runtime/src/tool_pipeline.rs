use std::time::{Instant, SystemTime, UNIX_EPOCH};

use loopagent_hooks::run_hook;
use loopagent_kernel::Kernel;
use loopagent_types::error::{LoopAgentError, Result};
use loopagent_types::hook::HookEvent;
use loopagent_types::tool::{ToolContext, ToolResult};
use loopagent_types::truncate::{needs_truncation, truncate_output};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::mode::AgentMode;

/// Maximum lines in a single tool result.
const MAX_RESULT_LINES: usize = 2000;
/// Maximum bytes in a single tool result (~25k tokens).
const MAX_RESULT_BYTES: usize = 100_000;

/// Execute a tool through the full pipeline: pre-hooks -> execute -> truncate -> post-hooks.
/// Permission checking is handled by agent_loop before calling this function.
pub async fn execute_tool(
    kernel: &Kernel,
    name: &str,
    input: Value,
    ctx: &ToolContext,
    _mode: &AgentMode,
) -> Result<ToolResult> {
    let tool = kernel
        .get_tool(name)
        .ok_or_else(|| LoopAgentError::Tool(loopagent_types::error::ToolError::NotFound(name.to_string())))?;

    // Run pre-hooks
    let pre_hooks = kernel
        .get_hooks(HookEvent::PreToolUse, Some(name));
    for hook_config in &pre_hooks {
        let hook_data = serde_json::json!({
            "tool_name": name,
            "tool_input": input,
        });
        match run_hook(hook_config, hook_data).await {
            Ok(result) => {
                if !result.is_success() {
                    warn!(tool = name, exit_code = result.exit_code, "pre-hook rejected tool execution");
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

    // Execute the tool
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

    // Truncate oversized output, saving full content to file
    let result = truncate_result(result, name);

    // Run post-hooks (sees truncated content)
    let post_hooks = kernel
        .get_hooks(HookEvent::PostToolUse, Some(name));
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

/// If the result exceeds size limits, save the full output to a temp file
/// and return the truncated version with a pointer to the saved file.
fn truncate_result(result: ToolResult, tool_name: &str) -> ToolResult {
    if !needs_truncation(&result.content, MAX_RESULT_LINES, MAX_RESULT_BYTES) {
        return result;
    }

    let saved_path = save_full_output(&result.content, tool_name);
    let mut truncated = truncate_output(&result.content, MAX_RESULT_LINES, MAX_RESULT_BYTES);
    if let Some(path) = saved_path {
        truncated.push_str(&format!("\n\n[Full output saved to: {path}]"));
    }
    warn!(
        tool = tool_name,
        original_bytes = result.content.len(),
        truncated_bytes = truncated.len(),
        "tool result truncated by pipeline"
    );
    ToolResult {
        content: truncated,
        is_error: result.is_error,
    }
}

/// Persist full output to `{temp_dir}/loopagent/tmp/` and return the file path.
fn save_full_output(content: &str, tool_name: &str) -> Option<String> {
    let tmp_dir = loopagent_config::tmp_dir();
    std::fs::create_dir_all(&tmp_dir).ok()?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();
    let filename = format!("tool_{tool_name}_{ts}.txt");
    let path = tmp_dir.join(&filename);
    std::fs::write(&path, content).ok()?;
    Some(path.to_string_lossy().into_owned())
}
