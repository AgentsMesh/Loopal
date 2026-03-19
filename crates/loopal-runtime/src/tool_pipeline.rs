use std::time::{Instant, SystemTime, UNIX_EPOCH};

use loopal_hooks::run_hook;
use loopal_kernel::Kernel;
use loopal_types::error::{LoopalError, Result};
use loopal_types::hook::HookEvent;
use loopal_types::tool::{ToolContext, ToolResult};
use loopal_types::truncate::{needs_truncation, truncate_output};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::mode::AgentMode;

const MAX_RESULT_LINES: usize = 2000;
const MAX_RESULT_BYTES: usize = 100_000;

/// Execute a tool through the full pipeline:
/// pre-hooks -> execute -> truncate -> post-hooks.
/// Sandbox enforcement is handled by the SandboxedTool decorator (precheck + execute).
pub async fn execute_tool(
    kernel: &Kernel,
    name: &str,
    input: Value,
    ctx: &ToolContext,
    _mode: &AgentMode,
) -> Result<ToolResult> {
    let tool = kernel
        .get_tool(name)
        .ok_or_else(|| LoopalError::Tool(loopal_types::error::ToolError::NotFound(name.to_string())))?;

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
                    warn!(tool = name, exit_code = result.exit_code, "pre-hook rejected");
                    return Ok(ToolResult::error(format!(
                        "Pre-hook rejected: {}", result.stderr.trim()
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

    let result = truncate_result(result, name);

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
    ToolResult { content: truncated, is_error: result.is_error }
}

fn save_full_output(content: &str, tool_name: &str) -> Option<String> {
    let tmp_dir = loopal_config::tmp_dir();
    std::fs::create_dir_all(&tmp_dir).ok()?;
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_millis();
    let filename = format!("tool_{tool_name}_{ts}.txt");
    let path = tmp_dir.join(&filename);
    std::fs::write(&path, content).ok()?;
    Some(path.to_string_lossy().into_owned())
}
