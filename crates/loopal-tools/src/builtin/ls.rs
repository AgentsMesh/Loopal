use async_trait::async_trait;
use loopal_types::error::LoopalError;
use loopal_types::permission::PermissionLevel;
use loopal_types::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct LsTool;

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "Ls"
    }

    fn description(&self) -> &str {
        "List directory contents with file type indicators."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to list (default: cwd)"
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let dir_path = match input["path"].as_str() {
            Some(p) => {
                let pb = PathBuf::from(p);
                if pb.is_absolute() { pb } else { ctx.cwd.join(pb) }
            }
            None => ctx.cwd.clone(),
        };

        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&dir_path).await.map_err(|e| {
            LoopalError::Tool(loopal_types::error::ToolError::ExecutionFailed(
                format!("Failed to read directory {}: {}", dir_path.display(), e),
            ))
        })?;

        while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
            LoopalError::Tool(loopal_types::error::ToolError::ExecutionFailed(
                format!("Failed to read entry: {}", e),
            ))
        })? {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry.file_type().await.ok();
            let indicator = match file_type {
                Some(ft) if ft.is_dir() => "/",
                Some(ft) if ft.is_symlink() => "@",
                _ => "",
            };
            entries.push(format!("{}{}", name, indicator));
        }

        entries.sort();

        if entries.is_empty() {
            Ok(ToolResult::success("(empty directory)"))
        } else {
            Ok(ToolResult::success(entries.join("\n")))
        }
    }
}
