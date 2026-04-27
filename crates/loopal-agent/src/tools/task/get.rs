//! `TaskGet` — fetch a task by ID with full description and dependencies.

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};

use crate::tools::collaboration::shared_extract::extract_shared;

pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "TaskGet"
    }
    fn description(&self) -> &str {
        "Get full details of a task by ID, including description, status, and dependencies. \
         Use this before starting work on a task to understand its full requirements."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": { "type": "string", "description": "Task ID" }
            },
            "required": ["taskId"]
        })
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        let id = input.get("taskId").and_then(|v| v.as_str()).unwrap_or("");
        match shared.task_store.get(id).await {
            Some(task) => {
                let json = serde_json::to_string_pretty(&task).unwrap_or_default();
                Ok(ToolResult::success(json))
            }
            None => Ok(ToolResult::error(format!("Task '{id}' not found"))),
        }
    }
}
