//! `TaskUpdate` — update task status, description, owner, or dependencies.

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};

use crate::task_patch::TaskPatch;
use crate::tools::collaboration::shared_extract::extract_shared;
use crate::tools::task::{parse_status, parse_string_array};

pub struct TaskUpdateTool;

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "TaskUpdate"
    }
    fn description(&self) -> &str {
        "Update a task's status, description, owner, or dependencies.\n\n\
         Status workflow: pending → in_progress → completed.\n\
         - Set 'in_progress' BEFORE beginning work on a task.\n\
         - Set 'completed' as soon as the task is done. Do not batch.\n\
         - Set 'deleted' to permanently remove a task that is no longer relevant.\n\n\
         ONLY mark a task as completed when you have FULLY accomplished it. \
         If you encounter errors or blockers, keep the task as in_progress."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": { "type": "string" },
                "status": { "type": "string", "enum": ["pending","in_progress","completed","deleted"] },
                "subject": { "type": "string" },
                "description": { "type": "string" },
                "activeForm": { "type": "string", "description": "Present continuous form shown in spinner when in_progress" },
                "owner": { "type": "string" },
                "addBlockedBy": { "type": "array", "items": { "type": "string" } },
                "addBlocks": { "type": "array", "items": { "type": "string" } },
                "metadata": { "type": "object" }
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

        let patch = TaskPatch {
            status: input
                .get("status")
                .and_then(|v| v.as_str())
                .and_then(parse_status),
            subject: input
                .get("subject")
                .and_then(|v| v.as_str())
                .map(String::from),
            description: input
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from),
            active_form: input
                .get("activeForm")
                .and_then(|v| v.as_str())
                .map(String::from),
            owner: input.get("owner").map(|v| v.as_str().map(String::from)),
            add_blocked_by: parse_string_array(&input, "addBlockedBy"),
            add_blocks: parse_string_array(&input, "addBlocks"),
            metadata: input.get("metadata").cloned(),
        };

        match shared.task_store.update(id, patch).await {
            Some(task) => {
                let json = serde_json::to_string_pretty(&task).unwrap_or_default();
                Ok(ToolResult::success(json))
            }
            None => Ok(ToolResult::error(format!("Task '{id}' not found"))),
        }
    }
}
