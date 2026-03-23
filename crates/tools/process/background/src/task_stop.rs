use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use crate::TaskStatus;

pub struct TaskStopTool;

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str {
        "TaskStop"
    }

    fn description(&self) -> &str {
        "Stop a running background task."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["task_id"],
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The background task ID to stop"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let task_id = input["task_id"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "task_id is required".into(),
            ))
        })?;

        let store = crate::store().lock().unwrap();
        let Some(task) = store.get(task_id) else {
            return Ok(ToolResult::error(format!("Task not found: {task_id}")));
        };

        let mut status = task.status.lock().unwrap();
        if *status != TaskStatus::Running {
            let current = format!("{:?}", *status);
            return Ok(ToolResult::success(format!(
                "Task already {current}: {task_id}"
            )));
        }

        // Kill the child process
        if let Some(child) = task.child.lock().unwrap().as_mut() {
            let _ = child.start_kill();
        }
        *status = TaskStatus::Failed;
        drop(status);
        drop(store);

        Ok(ToolResult::success(format!("Task stopped: {task_id}")))
    }
}
