//! Sandboxed Bash command executor.
//!
//! Connects the `wrap_command()` OS-level sandbox path with actual process execution.
//! Called by `SandboxedTool::execute()` when the inner tool is `Bash`.

use std::path::Path;
use std::time::Duration;

use loopal_types::error::{LoopalError, ToolError};
use loopal_types::sandbox::ResolvedPolicy;
use loopal_types::tool::ToolResult;
use loopal_types::truncate::truncate_output;
use tokio::process::Command;

use crate::command_wrapper::wrap_command;

const MAX_OUTPUT_LINES: usize = 2000;
const MAX_OUTPUT_BYTES: usize = 512_000;

/// Execute a bash command under OS-level sandbox enforcement.
///
/// Calls `wrap_command()` to build the sandboxed command (e.g. `sandbox-exec`
/// on macOS, `bwrap` on Linux), then spawns the process with sanitized env.
pub async fn execute_sandboxed_bash(
    policy: &ResolvedPolicy,
    command: &str,
    cwd: &Path,
    timeout_ms: u64,
) -> Result<ToolResult, LoopalError> {
    let sandboxed = wrap_command(policy, command, cwd);

    let mut cmd = Command::new(&sandboxed.program);
    cmd.args(&sandboxed.args)
        .current_dir(&sandboxed.cwd)
        .env_clear();
    for (k, v) in &sandboxed.env {
        cmd.env(k, v);
    }

    let result = tokio::time::timeout(Duration::from_millis(timeout_ms), cmd.output())
        .await
        .map_err(|_| LoopalError::Tool(ToolError::Timeout(timeout_ms)))?
        .map_err(|e| {
            LoopalError::Tool(ToolError::ExecutionFailed(format!(
                "sandbox exec failed: {e}"
            )))
        })?;

    format_output(result)
}

fn format_output(output: std::process::Output) -> Result<ToolResult, LoopalError> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut combined = String::new();
    if !stdout.is_empty() {
        combined.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }

    let truncated = truncate_output(&combined, MAX_OUTPUT_LINES, MAX_OUTPUT_BYTES);
    let is_error = !output.status.success();

    if is_error {
        let code = output.status.code().unwrap_or(-1);
        Ok(ToolResult::error(format!("Exit code: {code}\n{truncated}")))
    } else {
        Ok(ToolResult::success(truncated))
    }
}
