use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

use loopal_edit_core::omission_detector::detect_omissions;

pub struct EditTool;

#[derive(Deserialize, JsonSchema)]
pub struct EditParams {
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
    #[serde(default)]
    pub replace_all: Option<bool>,
}

#[async_trait]
impl TypedTool<EditParams> for EditTool {
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

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    async fn execute(
        &self,
        input: EditParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let replace_all = input.replace_all.unwrap_or(false);

        let omissions = detect_omissions(&input.new_string);
        if !omissions.is_empty() {
            return Ok(ToolResult::error(format!(
                "Omission detected in new_string. The following patterns suggest code was skipped: {}. Please provide the complete replacement text.",
                omissions.join(", ")
            )));
        }

        match ctx
            .backend
            .edit(
                &input.file_path,
                &input.old_string,
                &input.new_string,
                replace_all,
            )
            .await
        {
            Ok(_result) => Ok(ToolResult::success(format!(
                "Successfully edited {}",
                input.file_path
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
