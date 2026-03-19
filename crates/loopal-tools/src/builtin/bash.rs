use async_trait::async_trait;
use loopal_types::error::LoopalError;
use loopal_types::permission::PermissionLevel;
use loopal_types::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;

use crate::truncate::truncate_output;

/// BashTool executes shell commands. OS-level sandbox wrapping is handled
/// by the SandboxedTool decorator — BashTool itself is a plain executor.
pub struct BashTool;

impl Default for BashTool {
    fn default() -> Self {
        Self
    }
}

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_OUTPUT_LINES: usize = 2000;
const MAX_OUTPUT_BYTES: usize = 512_000;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command. Captures stdout and stderr."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["command"],
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 120000)"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Dangerous
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| {
                LoopalError::Tool(loopal_types::error::ToolError::InvalidInput(
                    "command is required".into(),
                ))
            })?;

        let timeout_ms = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);

        let result = execute_command(command, &ctx.cwd, timeout_ms).await;
        format_result(result, timeout_ms)
    }
}

async fn execute_command(
    command: &str,
    cwd: &std::path::Path,
    timeout_ms: u64,
) -> Result<std::process::Output, ExecError> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command).current_dir(cwd);

    tokio::time::timeout(Duration::from_millis(timeout_ms), cmd.output())
        .await
        .map_err(|_| ExecError::Timeout)?
        .map_err(ExecError::Io)
}

enum ExecError {
    Timeout,
    Io(std::io::Error),
}

fn format_result(
    result: Result<std::process::Output, ExecError>,
    timeout_ms: u64,
) -> Result<ToolResult, LoopalError> {
    match result {
        Ok(output) => {
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
        Err(ExecError::Io(e)) => {
            Ok(ToolResult::error(format!("Failed to execute command: {e}")))
        }
        Err(ExecError::Timeout) => Err(LoopalError::Tool(
            loopal_types::error::ToolError::Timeout(timeout_ms),
        )),
    }
}
