use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::edit::omission_detector::detect_omissions;

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

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopAgentError> {
        let file_path = input["file_path"]
            .as_str()
            .ok_or_else(|| {
                LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                    "file_path is required".into(),
                ))
            })?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| {
                LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                    "content is required".into(),
                ))
            })?;

        let path = resolve_path(file_path, &ctx.cwd);

        // Guard against path traversal for relative paths
        if !PathBuf::from(file_path).is_absolute() {
            // For new files, canonicalize the parent directory
            let check_path = if path.exists() {
                path.canonicalize().ok()
            } else {
                path.parent().and_then(|p| {
                    if p.exists() {
                        p.canonicalize().ok()
                    } else {
                        None
                    }
                })
            };
            if let Some(canonical) = check_path
                && !canonical.starts_with(&ctx.cwd) {
                    return Ok(ToolResult::error("path outside working directory"));
                }
        }

        // Check content for LLM omission patterns before writing
        let omissions = detect_omissions(content);
        if !omissions.is_empty() {
            return Ok(ToolResult::error(format!(
                "Omission detected in content. The following patterns suggest code was skipped: {}. Please provide the complete file content.",
                omissions.join(", ")
            )));
        }

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                LoopAgentError::Tool(loopagent_types::error::ToolError::ExecutionFailed(
                    format!("Failed to create directories: {}", e),
                ))
            })?;
        }

        tokio::fs::write(&path, content).await.map_err(|e| {
            LoopAgentError::Tool(loopagent_types::error::ToolError::ExecutionFailed(
                format!("Failed to write {}: {}", path.display(), e),
            ))
        })?;

        let bytes = content.len();
        Ok(ToolResult::success(format!(
            "Successfully wrote {} bytes to {}",
            bytes,
            path.display()
        )))
    }
}

fn resolve_path(file_path: &str, cwd: &std::path::Path) -> PathBuf {
    let p = PathBuf::from(file_path);
    if p.is_absolute() {
        p
    } else {
        cwd.join(p)
    }
}
