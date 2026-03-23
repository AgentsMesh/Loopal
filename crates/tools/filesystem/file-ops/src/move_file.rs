use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use crate::require_str;

pub struct MoveFileTool;

#[async_trait]
impl Tool for MoveFileTool {
    fn name(&self) -> &str {
        "MoveFile"
    }

    fn description(&self) -> &str {
        "Move or rename a file or directory."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["src", "dst"],
            "properties": {
                "src": { "type": "string", "description": "Source path" },
                "dst": { "type": "string", "description": "Destination path" }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let src_raw = require_str(&input, "src")?;
        let dst_raw = require_str(&input, "dst")?;

        // Validate source exists
        if let Err(e) = ctx.backend.file_info(src_raw).await {
            return Ok(ToolResult::error(e.to_string()));
        }

        // If dst is an existing directory, move src inside it
        let final_dst = match ctx.backend.file_info(dst_raw).await {
            Ok(info) if info.is_dir => {
                let src_path = std::path::Path::new(src_raw);
                let name = src_path.file_name().ok_or_else(|| {
                    LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                        "source has no file name".into(),
                    ))
                })?;
                let dst_path = std::path::Path::new(dst_raw).join(name);
                dst_path.to_string_lossy().into_owned()
            }
            _ => dst_raw.to_string(),
        };

        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(&final_dst).parent()
            && let Err(e) = ctx.backend.create_dir_all(&parent.to_string_lossy()).await
        {
            return Ok(ToolResult::error(e.to_string()));
        }

        // Try rename first; fall back to copy+remove for cross-device moves
        match ctx.backend.rename(src_raw, &final_dst).await {
            Ok(()) => {}
            Err(_) => {
                if let Err(e) = ctx.backend.copy(src_raw, &final_dst).await {
                    return Ok(ToolResult::error(e.to_string()));
                }
                if let Err(e) = ctx.backend.remove(src_raw).await {
                    return Ok(ToolResult::error(e.to_string()));
                }
            }
        }

        Ok(ToolResult::success(format!(
            "Moved {src_raw} → {final_dst}"
        )))
    }
}
