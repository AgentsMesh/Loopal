use async_trait::async_trait;
use loopal_edit_core::omission_detector::detect_omissions;
use loopal_edit_core::omission_message::format_omission_error;
use loopal_edit_core::search_replace::{SearchReplaceResult, search_replace};
use loopal_error::LoopalError;
use loopal_secret_runtime::{SECRET_REJECTION_MESSAGE, WIRE_REF_MARKER};
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

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

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    fn precheck(&self, input: &EditParams) -> Option<String> {
        if input.file_path.contains(WIRE_REF_MARKER)
            || input.old_string.contains(WIRE_REF_MARKER)
            || input.new_string.contains(WIRE_REF_MARKER)
        {
            return Some(SECRET_REJECTION_MESSAGE.into());
        }
        None
    }

    async fn execute(
        &self,
        input: EditParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let replace_all = input.replace_all.unwrap_or(false);

        let omissions = detect_omissions(&input.new_string);
        if !omissions.is_empty() {
            return Ok(ToolResult::error(format_omission_error(
                "new_string",
                &omissions,
            )));
        }

        match ctx.backend.resolve_path(&input.file_path, true) {
            Ok(path) => {
                let content = match ctx.backend.read_raw(&path).await {
                    Ok(c) => c,
                    Err(e) => return Ok(ToolResult::error(e.to_string())),
                };
                match search_replace(&content, &input.old_string, &input.new_string, replace_all) {
                    SearchReplaceResult::Ok(new_content) => {
                        match ctx.backend.write(&path, &new_content).await {
                            Ok(_) => Ok(ToolResult::success(format!(
                                "Successfully edited {}",
                                input.file_path
                            ))),
                            Err(e) => Ok(ToolResult::error(e.to_string())),
                        }
                    }
                    SearchReplaceResult::NotFound => Ok(ToolResult::error(format!(
                        "old_string not found in {}",
                        input.file_path
                    ))),
                    SearchReplaceResult::MultipleMatches(n) => Ok(ToolResult::error(format!(
                        "old_string found {n} times in {}; use replace_all=true or provide more context",
                        input.file_path
                    ))),
                }
            }
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
