use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, Tool, ToolContext, ToolResult};
use serde_json::{Value, json};

use loopal_edit_core::omission_detector::detect_omissions;

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        "Perform exact string replacement in a file.\n\
         - You must use Read at least once before editing. This tool will fail if you have not read the file first.\n\
         - Preserve the exact indentation (tabs/spaces) as shown in Read output.\n\
         - The edit will FAIL if old_string is not unique in the file. Provide a larger string with more surrounding context to make it unique, or use replace_all.\n\
         - ALWAYS prefer editing existing files. NEVER write new files unless explicitly required.\n\
         - Use replace_all for renaming variables or strings across the entire file."
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

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult, LoopalError> {
        let file_path = input["file_path"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "file_path is required".into(),
            ))
        })?;
        let old_string = input["old_string"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "old_string is required".into(),
            ))
        })?;
        let new_string = input["new_string"].as_str().ok_or_else(|| {
            LoopalError::Tool(loopal_error::ToolError::InvalidInput(
                "new_string is required".into(),
            ))
        })?;
        let replace_all = input["replace_all"].as_bool().unwrap_or(false);

        // Check new_string for LLM omission patterns before applying
        let omissions = detect_omissions(new_string);
        if !omissions.is_empty() {
            return Ok(ToolResult::error(format!(
                "Omission detected in new_string. The following patterns suggest code was skipped: {}. Please provide the complete replacement text.",
                omissions.join(", ")
            )));
        }

        // Backend handles: path resolution, traversal check, read, search_replace, atomic write
        match ctx
            .backend
            .edit(file_path, old_string, new_string, replace_all)
            .await
        {
            Ok(_result) => Ok(ToolResult::success(format!(
                "Successfully edited {file_path}"
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
