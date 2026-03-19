use async_trait::async_trait;
use loopal_types::error::LoopalError;
use loopal_types::permission::PermissionLevel;
use loopal_types::tool::{Tool, ToolContext, ToolResult};

use super::agent::extract_shared;
use crate::task_store::TaskPatch;
use crate::types::TaskStatus;

// --- Helper ---

fn parse_status(s: &str) -> Option<TaskStatus> {
    match s {
        "pending" => Some(TaskStatus::Pending),
        "in_progress" => Some(TaskStatus::InProgress),
        "completed" => Some(TaskStatus::Completed),
        "deleted" => Some(TaskStatus::Deleted),
        _ => None,
    }
}

// --- TaskCreate ---

pub struct TaskCreateTool;

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str { "TaskCreate" }
    fn description(&self) -> &str {
        "Create a new task in the shared task list."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subject": { "type": "string", "description": "Brief task title" },
                "description": { "type": "string", "description": "Detailed description" }
            },
            "required": ["subject", "description"]
        })
    }
    fn permission(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    async fn execute(
        &self, input: serde_json::Value, ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        let subject = input.get("subject").and_then(|v| v.as_str()).unwrap_or("");
        let desc = input.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let task = shared.task_store.create(subject, desc);
        let json = serde_json::to_string_pretty(&task).unwrap_or_default();
        Ok(ToolResult::success(json))
    }
}

// --- TaskUpdate ---

pub struct TaskUpdateTool;

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str { "TaskUpdate" }
    fn description(&self) -> &str {
        "Update an existing task (status, owner, dependencies, etc.)."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": { "type": "string" },
                "status": { "type": "string", "enum": ["pending","in_progress","completed","deleted"] },
                "subject": { "type": "string" },
                "description": { "type": "string" },
                "owner": { "type": "string" },
                "addBlockedBy": { "type": "array", "items": { "type": "string" } },
                "addBlocks": { "type": "array", "items": { "type": "string" } },
                "metadata": { "type": "object" }
            },
            "required": ["taskId"]
        })
    }
    fn permission(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    async fn execute(
        &self, input: serde_json::Value, ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        let id = input.get("taskId").and_then(|v| v.as_str()).unwrap_or("");

        let patch = TaskPatch {
            status: input.get("status").and_then(|v| v.as_str()).and_then(parse_status),
            subject: input.get("subject").and_then(|v| v.as_str()).map(String::from),
            description: input.get("description").and_then(|v| v.as_str()).map(String::from),
            owner: input.get("owner").map(|v| v.as_str().map(String::from)),
            add_blocked_by: parse_string_array(&input, "addBlockedBy"),
            add_blocks: parse_string_array(&input, "addBlocks"),
            metadata: input.get("metadata").cloned(),
        };

        match shared.task_store.update(id, patch) {
            Some(task) => {
                let json = serde_json::to_string_pretty(&task).unwrap_or_default();
                Ok(ToolResult::success(json))
            }
            None => Ok(ToolResult::error(format!("Task '{id}' not found"))),
        }
    }
}

// --- TaskList ---

pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str { "TaskList" }
    fn description(&self) -> &str { "List all tasks in the shared task list." }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }
    fn permission(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    async fn execute(
        &self, _input: serde_json::Value, ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        let tasks = shared.task_store.list();
        let json = serde_json::to_string_pretty(&tasks).unwrap_or_default();
        Ok(ToolResult::success(json))
    }
}

// --- TaskGet ---

pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str { "TaskGet" }
    fn description(&self) -> &str { "Get full details of a task by ID." }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": { "type": "string", "description": "Task ID" }
            },
            "required": ["taskId"]
        })
    }
    fn permission(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    async fn execute(
        &self, input: serde_json::Value, ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        let id = input.get("taskId").and_then(|v| v.as_str()).unwrap_or("");
        match shared.task_store.get(id) {
            Some(task) => {
                let json = serde_json::to_string_pretty(&task).unwrap_or_default();
                Ok(ToolResult::success(json))
            }
            None => Ok(ToolResult::error(format!("Task '{id}' not found"))),
        }
    }
}

fn parse_string_array(input: &serde_json::Value, key: &str) -> Vec<String> {
    input
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}
