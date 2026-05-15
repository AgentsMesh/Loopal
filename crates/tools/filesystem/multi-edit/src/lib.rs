use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use loopal_edit_core::omission_detector::detect_omissions;

pub struct MultiEditTool;

#[derive(Deserialize, JsonSchema)]
pub struct EditItem {
    pub old_string: String,
    pub new_string: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct MultiEditParams {
    pub file_path: String,
    pub edits: Vec<EditItem>,
}

#[async_trait]
impl TypedTool<MultiEditParams> for MultiEditTool {
    fn name(&self) -> &str {
        "MultiEdit"
    }

    fn description(&self) -> &str {
        "Apply multiple sequential edits to a single file atomically. \
         All edits succeed or none are applied."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    async fn execute(
        &self,
        input: MultiEditParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        if input.edits.is_empty() {
            return Ok(ToolResult::error("edits array must not be empty"));
        }

        let content = match ctx.backend.read_raw(&input.file_path).await {
            Ok(c) => c,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let mut current = content;
        for (i, edit) in input.edits.iter().enumerate() {
            let omissions = detect_omissions(&edit.new_string);
            if !omissions.is_empty() {
                return Ok(ToolResult::error(format!(
                    "Edit {i}: omission detected in new_string: {}",
                    omissions.join(", ")
                )));
            }

            let count = current.matches(&edit.old_string).count();
            match count {
                0 => {
                    return Ok(ToolResult::error(format!(
                        "Edit {i}: old_string not found in current content"
                    )));
                }
                1 => current = current.replacen(&edit.old_string, &edit.new_string, 1),
                n => {
                    return Ok(ToolResult::error(format!(
                        "Edit {i}: old_string found {n} times; must be unique"
                    )));
                }
            }
        }

        match ctx.backend.write(&input.file_path, &current).await {
            Ok(_) => Ok(ToolResult::success(format!(
                "Applied {} edit(s) to {}",
                input.edits.len(),
                input.file_path
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
