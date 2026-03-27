use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use loopal_edit_core::omission_detector::detect_omissions;

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates parent directories if needed."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["file_path", "content"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let file_path = input["file_path"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "file_path is required".into(),
            ))
        })?;
        let content = input["content"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "content is required".into(),
            ))
        })?;

        // Check content for LLM omission patterns before writing
        let omissions = detect_omissions(content);
        if !omissions.is_empty() {
            return Ok(ToolResult::error(format!(
                "Omission detected in content. The following patterns suggest code was skipped: {}. Please provide the complete file content.",
                omissions.join(", ")
            )));
        }

        // Backend handles: path resolution, traversal check, mkdir, atomic write
        match ctx.backend.write(file_path, content).await {
            Ok(result) => Ok(ToolResult::success(format!(
                "Successfully wrote {} bytes to {}",
                result.bytes_written, file_path
            ))
            .with_metadata(json!({"bytes_written": result.bytes_written}))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
