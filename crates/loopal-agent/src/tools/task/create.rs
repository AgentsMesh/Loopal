//! `TaskCreate` — create a structured task to track work.

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};

use crate::task_patch::TaskPatch;
use crate::tools::collaboration::shared_extract::extract_shared;

pub struct TaskCreateTool;

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "TaskCreate"
    }
    fn description(&self) -> &str {
        "Create a structured task to track your work. Use this proactively when starting \
         complex multi-step tasks — decompose them into subtasks BEFORE beginning implementation.\n\n\
         When to use:\n\
         - Complex tasks requiring 3+ steps\n\
         - Non-trivial tasks that need careful planning\n\
         - When the user provides multiple requests\n\
         - After receiving new instructions — capture requirements as tasks immediately\n\n\
         All tasks are created with status 'pending'. Use TaskUpdate to set 'in_progress' when you start \
         and 'completed' when you finish. Mark each task completed as soon as it is done — do not batch."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subject": { "type": "string", "description": "Brief task title" },
                "description": { "type": "string", "description": "Detailed description" },
                "activeForm": { "type": "string", "description": "Present continuous form shown in spinner when in_progress (e.g. 'Running tests')" },
                "metadata": { "type": "object", "description": "Arbitrary metadata to attach to the task" }
            },
            "required": ["subject", "description"]
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
        let subject = input.get("subject").and_then(|v| v.as_str()).unwrap_or("");
        let desc = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mut task = shared.task_store.create(subject, desc).await;
        // Apply optional fields after creation
        let mut needs_update = false;
        let mut patch = TaskPatch::default();
        if let Some(af) = input.get("activeForm").and_then(|v| v.as_str()) {
            patch.active_form = Some(af.to_string());
            needs_update = true;
        }
        if let Some(meta) = input.get("metadata") {
            patch.metadata = Some(meta.clone());
            needs_update = true;
        }
        if needs_update && let Some(updated) = shared.task_store.update(&task.id, patch).await {
            task = updated;
        }
        let json = serde_json::to_string_pretty(&task).unwrap_or_default();
        Ok(ToolResult::success(json))
    }
}
