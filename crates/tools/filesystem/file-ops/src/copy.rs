use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use crate::require_str;

pub struct CopyFileTool;

#[async_trait]
impl Tool for CopyFileTool {
    fn name(&self) -> &str {
        "CopyFile"
    }

    fn description(&self) -> &str {
        "Copy a file to a new location."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["src", "dst"],
            "properties": {
                "src": { "type": "string", "description": "Source file path" },
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

        // Validate source exists and is a file
        let src_info = match ctx.backend.file_info(src_raw).await {
            Ok(i) => i,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };
        if src_info.is_dir {
            return Ok(ToolResult::error(
                "source must be a file (use Bash for directory copies)",
            ));
        }

        // If dst is an existing directory, copy into it
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

        match ctx.backend.copy(src_raw, &final_dst).await {
            Ok(()) => Ok(ToolResult::success(format!(
                "Copied {src_raw} → {final_dst}"
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
