use async_trait::async_trait;
use loopal_error::{LoopalError, ToolIoError};
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult, truncate_output};
use serde_json::{Value, json};

use loopal_config::CommandDecision;
use loopal_sandbox::command_checker::check_command;
use loopal_sandbox::security_inspector::{SecurityVerdict, inspect_command};

/// BashTool executes shell commands. OS-level sandbox wrapping is handled
/// by the SandboxedTool decorator — BashTool itself is a plain executor.
pub struct BashTool;

impl Default for BashTool {
    fn default() -> Self {
        Self
    }
}

const DEFAULT_TIMEOUT_MS: u64 = 300_000;
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
                    "description": "Timeout in milliseconds (default: 300000, max: 600000)"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Run the command as a background task (default: false)"
                },
                "description": {
                    "type": "string",
                    "description": "Description of the background task"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Dangerous
    }

    fn precheck(&self, input: &Value) -> Option<String> {
        let cmd = input.get("command")?.as_str()?;
        // Structural pattern checks (fork bombs, destructive rm, device writes)
        if let CommandDecision::Deny(reason) = check_command(cmd) {
            return Some(reason);
        }
        // Semantic security checks (RCE, credential theft, system tampering)
        if let SecurityVerdict::Block(reason) = inspect_command(cmd) {
            return Some(reason);
        }
        None
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let command = input["command"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "command is required".into(),
            ))
        })?;

        let run_in_background = input["run_in_background"].as_bool().unwrap_or(false);

        if run_in_background {
            let desc = input["description"].as_str().unwrap_or(command);
            return match ctx.backend.exec_background(command, desc).await {
                Ok(task_id) => Ok(ToolResult::success(format!(
                    "Background task started: {task_id}"
                ))),
                Err(e) => Ok(ToolResult::error(e.to_string())),
            };
        }

        let timeout_ms = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);
        match ctx.backend.exec(command, timeout_ms).await {
            Ok(output) => Ok(format_exec_result(output)),
            Err(ToolIoError::Timeout(ms)) => {
                Err(LoopalError::Tool(loopal_error::ToolError::Timeout(ms)))
            }
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

fn format_exec_result(output: loopal_tool_api::backend_types::ExecResult) -> ToolResult {
    let mut combined = String::new();
    if !output.stdout.is_empty() {
        combined.push_str(&output.stdout);
    }
    if !output.stderr.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&output.stderr);
    }

    let truncated = truncate_output(&combined, MAX_OUTPUT_LINES, MAX_OUTPUT_BYTES);

    if output.exit_code != 0 {
        ToolResult::error(format!("Exit code: {}\n{truncated}", output.exit_code))
    } else {
        ToolResult::success(truncated)
    }
}
