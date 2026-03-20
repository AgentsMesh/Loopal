use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::edit::omission_detector::detect_omissions;

pub struct MultiEditTool;

#[async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "MultiEdit"
    }

    fn description(&self) -> &str {
        "Apply multiple sequential edits to a single file atomically. \
         All edits succeed or none are applied."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["file_path", "edits"],
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "edits": {
                    "type": "array",
                    "description": "Ordered list of search-and-replace edits",
                    "items": {
                        "type": "object",
                        "required": ["old_string", "new_string"],
                        "properties": {
                            "old_string": {
                                "type": "string",
                                "description": "Exact string to find"
                            },
                            "new_string": {
                                "type": "string",
                                "description": "Replacement string"
                            }
                        }
                    }
                }
            }
        })
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Supervised
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let file_path = require_str(&input, "file_path")?;
        let edits = input["edits"]
            .as_array()
            .ok_or_else(|| tool_input_err("edits array is required"))?;

        if edits.is_empty() {
            return Ok(ToolResult::error("edits array must not be empty"));
        }

        let path = resolve_path(file_path, &ctx.cwd);

        // Traversal protection
        let normalized = path.canonicalize().unwrap_or_else(|_| path.clone());
        let cwd_canonical = ctx.cwd.canonicalize().unwrap_or_else(|_| ctx.cwd.clone());
        if !normalized.starts_with(&cwd_canonical) {
            return Ok(ToolResult::error("path outside working directory"));
        }

        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
                format!("Failed to read {}: {e}", path.display()),
            ))
        })?;

        // Apply all edits on an in-memory copy
        let mut current = content;
        for (i, edit) in edits.iter().enumerate() {
            let old_str = edit["old_string"].as_str().unwrap_or("");
            let new_str = edit["new_string"].as_str().unwrap_or("");

            let omissions = detect_omissions(new_str);
            if !omissions.is_empty() {
                return Ok(ToolResult::error(format!(
                    "Edit {i}: omission detected in new_string: {}",
                    omissions.join(", ")
                )));
            }

            match search_replace(&current, old_str, new_str) {
                ReplaceOutcome::Ok(updated) => current = updated,
                ReplaceOutcome::NotFound => {
                    return Ok(ToolResult::error(format!(
                        "Edit {i}: old_string not found in current content"
                    )));
                }
                ReplaceOutcome::MultipleMatches(count) => {
                    return Ok(ToolResult::error(format!(
                        "Edit {i}: old_string found {count} times; must be unique"
                    )));
                }
            }
        }

        // Atomic write — only reached when all edits succeeded
        tokio::fs::write(&path, &current).await.map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(
                format!("Failed to write {}: {e}", path.display()),
            ))
        })?;

        Ok(ToolResult::success(format!(
            "Applied {} edit(s) to {}",
            edits.len(),
            path.display()
        )))
    }
}

// --- helpers -----------------------------------------------------------------

enum ReplaceOutcome {
    Ok(String),
    NotFound,
    MultipleMatches(usize),
}

fn search_replace(content: &str, old: &str, new: &str) -> ReplaceOutcome {
    let count = content.matches(old).count();
    match count {
        0 => ReplaceOutcome::NotFound,
        1 => ReplaceOutcome::Ok(content.replacen(old, new, 1)),
        n => ReplaceOutcome::MultipleMatches(n),
    }
}

fn resolve_path(file_path: &str, cwd: &std::path::Path) -> PathBuf {
    let p = PathBuf::from(file_path);
    if p.is_absolute() { p } else { cwd.join(p) }
}

fn require_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, LoopalError> {
    input[key]
        .as_str()
        .ok_or_else(|| tool_input_err(&format!("{key} is required")))
}

fn tool_input_err(msg: &str) -> LoopalError {
    LoopalError::Tool(loopal_error::ToolError::InvalidInput(msg.into()))
}
