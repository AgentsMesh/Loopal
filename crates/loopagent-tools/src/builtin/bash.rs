use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;

use crate::truncate::truncate_output;

pub struct BashTool;

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

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopAgentError> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| {
                LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                    "command is required".into(),
                ))
            })?;

        let timeout_ms = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);

        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&ctx.cwd)
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
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
                    Ok(ToolResult::error(format!(
                        "Exit code: {}\n{}",
                        code, truncated
                    )))
                } else {
                    Ok(ToolResult::success(truncated))
                }
            }
            Ok(Err(e)) => Ok(ToolResult::error(format!("Failed to execute command: {}", e))),
            Err(_) => Err(LoopAgentError::Tool(
                loopagent_types::error::ToolError::Timeout(timeout_ms),
            )),
        }
    }
}
