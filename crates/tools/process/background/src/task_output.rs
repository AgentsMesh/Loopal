use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

use crate::TaskStatus;

pub struct TaskOutputTool;

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const POLL_INTERVAL_MS: u64 = 100;

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        "TaskOutput"
    }

    fn description(&self) -> &str {
        "Read the output of a background task. Can block until completion or return immediately."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["task_id"],
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The background task ID to read output from"
                },
                "block": {
                    "type": "boolean",
                    "description": "Whether to block until the task completes (default: true)"
                },
                "timeout": {
                    "type": "number",
                    "description": "Timeout in milliseconds (default: 30000, max: 600000)"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let task_id = input["task_id"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "task_id is required".into(),
            ))
        })?;

        let block = input["block"].as_bool().unwrap_or(true);
        let timeout_ms = input["timeout"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);

        let (output_buf, exit_code_buf, status_buf) = {
            let store = crate::store().lock().unwrap();
            let Some(task) = store.get(task_id) else {
                return Ok(ToolResult::error(format!("Task not found: {task_id}")));
            };
            (
                Arc::clone(&task.output),
                Arc::clone(&task.exit_code),
                Arc::clone(&task.status),
            )
        };

        if block {
            let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
            loop {
                let status = status_buf.lock().unwrap().clone();
                if status != TaskStatus::Running {
                    break;
                }
                if tokio::time::Instant::now() >= deadline {
                    let output = output_buf.lock().unwrap().clone();
                    return Ok(ToolResult::success(format!(
                        "{output}\n[Status: Running (timed out waiting)]"
                    )));
                }
                tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
            }
        }

        let output = output_buf.lock().unwrap().clone();
        let status = status_buf.lock().unwrap().clone();
        let exit_code = *exit_code_buf.lock().unwrap();
        let status_line = format_status(&status, exit_code);

        Ok(ToolResult::success(format!("{output}\n{status_line}")))
    }
}

fn format_status(status: &TaskStatus, exit_code: Option<i32>) -> String {
    match status {
        TaskStatus::Running => "[Status: Running]".into(),
        TaskStatus::Completed => match exit_code {
            Some(code) => format!("[Status: Completed, Exit code: {code}]"),
            None => "[Status: Completed]".into(),
        },
        TaskStatus::Failed => match exit_code {
            Some(code) => format!("[Status: Failed, Exit code: {code}]"),
            None => "[Status: Failed]".into(),
        },
    }
}
