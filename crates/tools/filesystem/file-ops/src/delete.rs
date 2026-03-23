use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use crate::require_str;

pub struct DeleteTool;

#[async_trait]
impl Tool for DeleteTool {
    fn name(&self) -> &str {
        "Delete"
    }

    fn description(&self) -> &str {
        "Delete a file or directory."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Path to delete" }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let path_raw = require_str(&input, "path")?;

        // Check existence and type
        let info = match ctx.backend.file_info(path_raw).await {
            Ok(i) => i,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let kind = if info.is_dir { "directory" } else { "file" };

        match ctx.backend.remove(path_raw).await {
            Ok(()) => Ok(ToolResult::success(format!("Deleted {path_raw} ({kind})"))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
