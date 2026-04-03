//! Bash tool — execute shell commands with integrated background process management.
//!
//! Dispatch: `process_id` present → operate on background process;
//! `command` present → execute command (foreground or background).
//!
//! Foreground commands that exceed their timeout are automatically converted
//! to background tasks (via the streaming execution path) so the LLM can
//! check on them later with `process_id`.

mod bg_convert;
mod bg_monitor;
mod bg_ops;
mod format;

use async_trait::async_trait;
use loopal_error::{LoopalError, ToolIoError};
use loopal_tool_api::{ExecOutcome, PermissionLevel, TimeoutSecs, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use loopal_config::CommandDecision;
use loopal_sandbox::command_checker::check_command;
use loopal_sandbox::security_inspector::{SecurityVerdict, inspect_command};

pub struct BashTool;

impl Default for BashTool {
    fn default() -> Self {
        Self
    }
}

const DEFAULT_TIMEOUT_SECS: u64 = 300;
const DEFAULT_BG_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_MS: u64 = 600_000;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command, or manage a background process.\n\
         - Run command: provide `command`\n\
         - Background: provide `command` + `run_in_background: true`\n\
         - Get output: provide `process_id` (blocks until done by default)\n\
         - Stop: provide `process_id` + `stop: true`"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string" },
                "timeout": { "type": "integer", "description": "Timeout in seconds" },
                "run_in_background": { "type": "boolean" },
                "description": { "type": "string" },
                "process_id": { "type": "string" },
                "block": { "type": "boolean" },
                "stop": { "type": "boolean" }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Dangerous
    }

    fn precheck(&self, input: &Value) -> Option<String> {
        let cmd = input.get("command")?.as_str()?;
        if let CommandDecision::Deny(reason) = check_command(cmd) {
            return Some(reason);
        }
        if let SecurityVerdict::Block(reason) = inspect_command(cmd) {
            return Some(reason);
        }
        None
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        // Route: process_id → background ops, command → execute
        if let Some(pid) = input["process_id"].as_str() {
            if input["stop"].as_bool().unwrap_or(false) {
                return Ok(bg_ops::bg_stop(pid));
            }
            let block = input["block"].as_bool().unwrap_or(true);
            let timeout = TimeoutSecs::from_tool_input(&input, DEFAULT_BG_TIMEOUT_SECS);
            return Ok(
                bg_ops::bg_output(pid, block, timeout.to_millis_clamped(MAX_TIMEOUT_MS)).await,
            );
        }

        let command = input["command"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "provide 'command' or 'process_id'".into(),
            ))
        })?;

        if input["run_in_background"].as_bool().unwrap_or(false) {
            let desc = input["description"].as_str().unwrap_or(command);
            return match ctx.backend.exec_background(command).await {
                Ok(handle) => {
                    let task_id = bg_convert::register_spawned(handle, desc)
                        .unwrap_or_else(|| "(unknown)".into());
                    Ok(ToolResult::success(format!(
                        "Background process started.\nprocess_id: {task_id}"
                    )))
                }
                Err(e) => Ok(ToolResult::error(e.to_string())),
            };
        }

        exec_foreground(command, &input, ctx).await
    }
}

/// Execute a foreground command; on timeout the process is moved to background.
async fn exec_foreground(
    command: &str,
    input: &Value,
    ctx: &ToolContext,
) -> Result<ToolResult, LoopalError> {
    let timeout_ms =
        TimeoutSecs::from_tool_input(input, DEFAULT_TIMEOUT_SECS).to_millis_clamped(MAX_TIMEOUT_MS);

    let exec_result = if let Some(ref tail) = ctx.output_tail {
        ctx.backend
            .exec_streaming(command, timeout_ms, tail.clone())
            .await
    } else {
        ctx.backend
            .exec(command, timeout_ms)
            .await
            .map(ExecOutcome::Completed)
    };

    match exec_result {
        Ok(ExecOutcome::Completed(output)) => Ok(format::format_exec_result(output)),
        Ok(ExecOutcome::TimedOut {
            timeout_ms,
            partial_output,
            handle,
        }) => {
            let task_id = bg_convert::register(handle, command)
                .unwrap_or_else(|| "(unknown)".into());
            Ok(format::format_converted_to_background(
                &task_id,
                timeout_ms,
                &partial_output,
            ))
        }
        Err(ToolIoError::Timeout(ms)) => {
            Err(LoopalError::Tool(loopal_error::ToolError::Timeout(ms)))
        }
        Err(e) => Ok(ToolResult::error(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_secs_converts_to_millis() {
        let t = TimeoutSecs::from_tool_input(&json!({"timeout": 120}), 0);
        assert_eq!(t.as_secs(), 120);
        assert_eq!(t.to_millis_clamped(MAX_TIMEOUT_MS), 120_000);
    }

    #[test]
    fn timeout_secs_clamps_to_max() {
        let t = TimeoutSecs::from_tool_input(&json!({"timeout": 700}), 0);
        assert_eq!(t.to_millis_clamped(MAX_TIMEOUT_MS), MAX_TIMEOUT_MS);
    }

    #[test]
    fn timeout_secs_uses_default_when_missing() {
        let t = TimeoutSecs::from_tool_input(&json!({}), DEFAULT_TIMEOUT_SECS);
        assert_eq!(t.as_secs(), DEFAULT_TIMEOUT_SECS);
        let t2 = TimeoutSecs::from_tool_input(&json!({"command": "ls"}), 42);
        assert_eq!(t2.as_secs(), 42);
    }

    #[test]
    fn timeout_secs_zero_yields_zero() {
        let t = TimeoutSecs::from_tool_input(&json!({"timeout": 0}), DEFAULT_TIMEOUT_SECS);
        assert_eq!(t.as_secs(), 0);
        assert_eq!(t.to_millis_clamped(MAX_TIMEOUT_MS), 0);
    }

    #[test]
    fn timeout_secs_display() {
        assert_eq!(TimeoutSecs::new(300).to_string(), "300s");
        assert_eq!(TimeoutSecs::new(0).to_string(), "0s");
    }
}
