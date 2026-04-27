//! `TaskList` — list all tasks to see current progress.

use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};

use crate::tools::collaboration::shared_extract::extract_shared;

pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "TaskList"
    }
    fn description(&self) -> &str {
        "List all tasks to see current progress. Check this after completing each task \
         to find what remains and identify newly unblocked work."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "object", "properties": {} })
    }
    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(
        &self,
        _input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let shared = extract_shared(ctx)?;
        let tasks = shared.task_store.list().await;
        let json = serde_json::to_string_pretty(&tasks).unwrap_or_default();
        Ok(ToolResult::success(json))
    }
}
