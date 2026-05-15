use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use loopal_tool_invocation::ToolResultMetadata;
use schemars::JsonSchema;
use serde::Deserialize;

use loopal_edit_core::omission_detector::detect_omissions;

pub struct WriteTool;

#[derive(Deserialize, JsonSchema)]
pub struct WriteParams {
    pub file_path: String,
    pub content: String,
}

#[async_trait]
impl TypedTool<WriteParams> for WriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates parent directories if needed.\n\
         - If the file already exists, you MUST use Read first to see its current contents. This tool will fail if you did not read it first.\n\
         - Prefer the Edit tool for modifying existing files — it only sends the diff.\n\
         - NEVER create documentation files (*.md) or README files unless explicitly requested.\n\
         - Only use emojis if the user explicitly requests it."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    async fn execute(
        &self,
        input: WriteParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let file_path = &input.file_path;
        let content = &input.content;

        let omissions = detect_omissions(content);
        if !omissions.is_empty() {
            return Ok(ToolResult::error(format!(
                "Omission detected in content. The following patterns suggest code was skipped: {}. Please provide the complete file content.",
                omissions.join(", ")
            )));
        }

        match ctx.backend.write(file_path, content).await {
            Ok(result) => Ok(ToolResult::success(format!(
                "Successfully wrote {} bytes to {}",
                result.bytes_written, file_path
            ))
            .with_metadata(ToolResultMetadata::bytes_written(
                result.bytes_written as u64,
            ))),
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
