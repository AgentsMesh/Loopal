use async_trait::async_trait;
use loopagent_types::error::LoopAgentError;
use loopagent_types::permission::PermissionLevel;
use loopagent_types::tool::{Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::edit::omission_detector::detect_omissions;
use crate::edit::search_replace::{search_replace, SearchReplaceResult};

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        "Perform exact string replacement in a file. The old_string must be unique unless replace_all is true."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["file_path", "old_string", "new_string"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to search for"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)"
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
        let old_string = input["old_string"]
            .as_str()
            .ok_or_else(|| {
                LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                    "old_string is required".into(),
                ))
            })?;
        let new_string = input["new_string"]
            .as_str()
            .ok_or_else(|| {
                LoopAgentError::Tool(loopagent_types::error::ToolError::InvalidInput(
                    "new_string is required".into(),
                ))
            })?;
        let replace_all = input["replace_all"].as_bool().unwrap_or(false);

        let path = resolve_path(file_path, &ctx.cwd);

        // Guard against path traversal for relative paths
        if !PathBuf::from(file_path).is_absolute()
            && let Ok(canonical) = path.canonicalize()
            && !canonical.starts_with(&ctx.cwd) {
                return Ok(ToolResult::error("path outside working directory"));
            }

        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            LoopAgentError::Tool(loopagent_types::error::ToolError::ExecutionFailed(
                format!("Failed to read {}: {}", path.display(), e),
            ))
        })?;

        // Check new_string for LLM omission patterns before applying
        let omissions = detect_omissions(new_string);
        if !omissions.is_empty() {
            return Ok(ToolResult::error(format!(
                "Omission detected in new_string. The following patterns suggest code was skipped: {}. Please provide the complete replacement text.",
                omissions.join(", ")
            )));
        }

        match search_replace(&content, old_string, new_string, replace_all) {
            SearchReplaceResult::Ok(new_content) => {
                tokio::fs::write(&path, &new_content).await.map_err(|e| {
                    LoopAgentError::Tool(loopagent_types::error::ToolError::ExecutionFailed(
                        format!("Failed to write {}: {}", path.display(), e),
                    ))
                })?;
                Ok(ToolResult::success(format!(
                    "Successfully edited {}",
                    path.display()
                )))
            }
            SearchReplaceResult::NotFound => Ok(ToolResult::error(format!(
                "old_string not found in {}",
                path.display()
            ))),
            SearchReplaceResult::MultipleMatches(count) => Ok(ToolResult::error(format!(
                "old_string found {} times in {}. Use replace_all=true to replace all, or provide more context to make it unique.",
                count,
                path.display()
            ))),
        }
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
