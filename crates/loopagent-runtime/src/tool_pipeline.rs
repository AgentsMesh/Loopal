use std::time::Instant;

use loopagent_hooks::run_hook;
use loopagent_kernel::Kernel;
use loopagent_types::error::{LoopAgentError, Result};
use loopagent_types::hook::HookEvent;
use loopagent_types::tool::{ToolContext, ToolResult};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::mode::AgentMode;

/// Execute a tool through the full pipeline: pre-hooks -> execute -> post-hooks.
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

    // Run post-hooks
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
