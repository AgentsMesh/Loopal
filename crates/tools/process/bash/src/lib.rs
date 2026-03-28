//! Bash tool — execute shell commands with integrated background process management.
//!
//! Dispatch: `process_id` present → operate on background process;
//! `command` present → execute command (foreground or background).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loopal_error::{LoopalError, ToolIoError};
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult, truncate_output};
use serde_json::{Value, json};

use loopal_config::CommandDecision;
use loopal_sandbox::command_checker::check_command;
use loopal_sandbox::security_inspector::{SecurityVerdict, inspect_command};
use loopal_tool_background::TaskStatus;

pub struct BashTool;

impl Default for BashTool {
    fn default() -> Self {
        Self
    }
}

const DEFAULT_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_BG_TIMEOUT_MS: u64 = 30_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const MAX_OUTPUT_LINES: usize = 2000;
const MAX_OUTPUT_BYTES: usize = 512_000;

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
                "timeout": { "type": "integer" },
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
                return Ok(bg_stop(pid));
            }
            let block = input["block"].as_bool().unwrap_or(true);
            let timeout = input["timeout"]
                .as_u64()
                .unwrap_or(DEFAULT_BG_TIMEOUT_MS)
                .min(MAX_TIMEOUT_MS);
            return Ok(bg_output(pid, block, timeout).await);
        }

        let command = input["command"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "provide 'command' or 'process_id'".into(),
            ))
        })?;

        if input["run_in_background"].as_bool().unwrap_or(false) {
            let desc = input["description"].as_str().unwrap_or(command);
            return match ctx.backend.exec_background(command, desc).await {
                Ok(id) => Ok(ToolResult::success(format!(
                    "Background process started.\nprocess_id: {id}"
                ))),
                Err(e) => Ok(ToolResult::error(e.to_string())),
            };
        }

        let timeout_ms = input["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);
        let exec_result = if let Some(ref tail) = ctx.output_tail {
            ctx.backend
                .exec_streaming(command, timeout_ms, tail.clone())
                .await
        } else {
            ctx.backend.exec(command, timeout_ms).await
        };
        match exec_result {
            Ok(output) => Ok(format_exec_result(output)),
            Err(ToolIoError::Timeout(ms)) => {
                Err(LoopalError::Tool(loopal_error::ToolError::Timeout(ms)))
            }
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

/// Read output from a background process (block or non-blocking).
async fn bg_output(process_id: &str, block: bool, timeout_ms: u64) -> ToolResult {
    let (output_buf, exit_code_buf, status_buf, mut watch_rx) = {
        let store = loopal_tool_background::store().lock().unwrap();
        let Some(task) = store.get(process_id) else {
            return ToolResult::error(format!("Process not found: {process_id}"));
        };
        (
            Arc::clone(&task.output),
            Arc::clone(&task.exit_code),
            Arc::clone(&task.status),
            task.status_watch.clone(),
        )
    };

    if block {
        let deadline = Duration::from_millis(timeout_ms);
        let wait = async {
            loop {
                if *watch_rx.borrow() != TaskStatus::Running {
                    return;
                }
                if watch_rx.changed().await.is_err() {
                    return;
                }
            }
        };
        if tokio::time::timeout(deadline, wait).await.is_err() {
            let output = output_buf.lock().unwrap().clone();
            return ToolResult::success(format!("{output}\n[Status: Running (timed out waiting)]"));
        }
    }

    let output = output_buf.lock().unwrap().clone();
    let status = status_buf.lock().unwrap().clone();
    let exit_code = *exit_code_buf.lock().unwrap();
    let status_line = match status {
        TaskStatus::Running => "[Status: Running]",
        TaskStatus::Completed => match exit_code {
            Some(c) => return ToolResult::success(format!("{output}\n[Completed, exit {c}]")),
            None => "[Status: Completed]",
        },
        TaskStatus::Failed => match exit_code {
            Some(c) => return ToolResult::error(format!("{output}\n[Failed, exit {c}]")),
            None => "[Status: Failed]",
        },
    };
    ToolResult::success(format!("{output}\n{status_line}"))
}

/// Stop a background process.
fn bg_stop(process_id: &str) -> ToolResult {
    let store = loopal_tool_background::store().lock().unwrap();
    let Some(task) = store.get(process_id) else {
        return ToolResult::error(format!("Process not found: {process_id}"));
    };
    let mut status = task.status.lock().unwrap();
    if *status != TaskStatus::Running {
        return ToolResult::success(format!("Process already {:?}: {process_id}", *status));
    }
    if let Some(child) = task.child.lock().unwrap().as_mut() {
        let _ = child.start_kill();
    }
    *status = TaskStatus::Failed;
    ToolResult::success(format!("Process stopped: {process_id}"))
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
