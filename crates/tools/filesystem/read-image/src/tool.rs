use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub struct ReadImageTool;

#[derive(Deserialize, JsonSchema)]
pub struct ReadImageParams {
    pub file_path: String,
}

#[async_trait]
impl TypedTool<ReadImageParams> for ReadImageTool {
    fn name(&self) -> &str {
        "ReadImage"
    }

    fn description(&self) -> &str {
        "Reads an image file from the local filesystem and presents it visually to the model.\n\
         Supported formats: PNG, JPEG, GIF, WEBP.\n\
         Constraints: file ≤ 10 MB, dimensions ≤ 8192×8192.\n\
         The file_path parameter must be an absolute path.\n\
         For text/code files, use Read instead. For PDF, use ReadPdf. For HTML, use ReadHtml."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: ReadImageParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        match ctx.backend.read_image(&input.file_path).await {
            Ok(img) => {
                let summary = format!(
                    "Loaded {} ({}×{}, {} bytes).",
                    img.media_type, img.dimensions.0, img.dimensions.1, img.byte_size
                );
                Ok(ToolResult::success(summary).with_image(img))
            }
            Err(e) => Ok(ToolResult::error(e.to_string())),
        }
    }
}
