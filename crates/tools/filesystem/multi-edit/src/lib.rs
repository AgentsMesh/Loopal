use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use loopal_edit_core::omission_detector::detect_omissions;

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
                            "old_string": { "type": "string" },
                            "new_string": { "type": "string" }
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
            .ok_or_else(|| tool_err("edits array is required"))?;

        if edits.is_empty() {
            return Ok(ToolResult::error("edits array must not be empty"));
        }

        // Read raw content via backend (path check + size limit + binary detect)
        let content = match ctx.backend.read_raw(file_path).await {
            Ok(c) => c,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

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

            let count = current.matches(old_str).count();
            match count {
                0 => {
                    return Ok(ToolResult::error(format!(
                        "Edit {i}: old_string not found in current content"
                    )));
                }
                1 => current = current.replacen(old_str, new_str, 1),
                n => {
                    return Ok(ToolResult::error(format!(
                        "Edit {i}: old_string found {n} times; must be unique"
                    )));
                }
            }
        }

        // Atomic write via backend
        match ctx.backend.write(file_path, &current).await {
            Ok(_) => Ok(ToolResult::success(format!(
                "Applied {} edit(s) to {file_path}",
                edits.len()
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}

fn require_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, LoopalError> {
    input[key]
        .as_str()
        .ok_or_else(|| tool_err(&format!("{key} is required")))
}

fn tool_err(msg: &str) -> LoopalError {
    LoopalError::Tool(loopal_error::ToolError::InvalidInput(msg.into()))
}
