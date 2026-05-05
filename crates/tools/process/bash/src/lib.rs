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
pub mod format;
pub mod strategy;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{LoopalError, ToolIoError};
use loopal_tool_api::{ExecOutcome, PermissionLevel, TimeoutSecs, Tool, ToolContext, ToolResult};
use loopal_tool_background::BackgroundTaskStore;
use serde_json::{Value, json};

use loopal_config::CommandDecision;
use loopal_sandbox::command_checker::check_command;
use loopal_sandbox::security_inspector::{SecurityVerdict, inspect_command};

pub struct BashTool {
    store: Arc<BackgroundTaskStore>,
}

impl BashTool {
    pub fn new(store: Arc<BackgroundTaskStore>) -> Self {
        Self { store }
    }
}

const DEFAULT_TIMEOUT_SECS: u64 = 300;
const DEFAULT_BG_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT: Duration = Duration::from_secs(600);

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        "Executes a given bash command and returns its output.\n\n\
         The working directory persists between commands, but shell state does not.\n\n\
         IMPORTANT: Avoid using this tool to run `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, \
         or `echo` commands, unless explicitly instructed or after you have verified that a dedicated tool \
         cannot accomplish your task. Instead, use the appropriate dedicated tool:\n\
         - File search: Use Glob (NOT find or ls)\n\
         - Content search: Use Grep (NOT grep or rg)\n\
         - Read files: Use Read (NOT cat/head/tail)\n\
         - Edit files: Use Edit (NOT sed/awk)\n\
         - Write files: Use Write (NOT echo >/cat <<EOF)\n\n\
         # Instructions\n\
         - Always quote file paths that contain spaces with double quotes.\n\
         - Try to maintain your current working directory by using absolute paths and avoiding `cd`.\n\
         - You may specify an optional timeout in seconds (up to 600s / 10 minutes). Default timeout is 300s (5 minutes).\n\
         - Use `run_in_background` for long-running commands. You will be notified when they finish — do not poll.\n\
         - When issuing multiple commands:\n\
           - If independent and can run in parallel, make multiple Bash tool calls in a single message.\n\
           - If dependent, chain them with '&&' in a single Bash call.\n\
           - DO NOT use newlines to separate commands.\n\
         - Avoid unnecessary `sleep` commands — diagnose root causes instead of retry loops.\n\
         - For git commands: prefer new commits over amending; never skip hooks (--no-verify) unless asked."
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
        if let Some(pid) = input["process_id"].as_str() {
            if input["stop"].as_bool().unwrap_or(false) {
                return Ok(bg_ops::bg_stop(&self.store, pid));
            }
            let block = input["block"].as_bool().unwrap_or(true);
            let timeout = TimeoutSecs::from_tool_input(&input, DEFAULT_BG_TIMEOUT_SECS);
            return Ok(bg_ops::bg_output(
                &self.store,
                pid,
                block,
                timeout.to_duration_clamped(MAX_TIMEOUT),
            )
            .await);
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
                    let task_id = bg_convert::register_spawned(&self.store, handle, desc)
                        .unwrap_or_else(|| "(unknown)".into());
                    Ok(ToolResult::success(format!(
                        "Background process started.\nprocess_id: {task_id}"
                    )))
                }
                Err(e) => Ok(ToolResult::error(e.to_string())),
            };
        }

        exec_foreground(&self.store, command, &input, ctx).await
    }
}

async fn exec_foreground(
    store: &BackgroundTaskStore,
    command: &str,
    input: &Value,
    ctx: &ToolContext,
) -> Result<ToolResult, LoopalError> {
    let timeout =
        TimeoutSecs::from_tool_input(input, DEFAULT_TIMEOUT_SECS).to_duration_clamped(MAX_TIMEOUT);

    let exec_result = if let Some(ref tail) = ctx.output_tail {
        ctx.backend
            .exec_streaming(command, timeout, tail.clone())
            .await
    } else {
        ctx.backend
            .exec(command, timeout)
            .await
            .map(ExecOutcome::Completed)
    };

    match exec_result {
        Ok(ExecOutcome::Completed(output)) => Ok(format::format_exec_result(output, command)),
        Ok(ExecOutcome::TimedOut {
            timeout,
            partial_output,
            handle,
        }) => {
            let task_id =
                bg_convert::register(store, handle, command).unwrap_or_else(|| "(unknown)".into());
            Ok(format::format_converted_to_background(
                &task_id,
                timeout,
                &partial_output,
            ))
        }
        Err(ToolIoError::Timeout(d)) => Err(LoopalError::Tool(loopal_error::ToolError::Timeout(d))),
        Err(e) => Ok(ToolResult::error(e.to_string())),
    }
}

#[cfg(test)]
#[path = "timeout_test.rs"]
mod timeout_test;
