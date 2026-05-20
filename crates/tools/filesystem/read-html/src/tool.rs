use async_trait::async_trait;
use loopal_error::LoopalError;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult, TypedTool};
use schemars::JsonSchema;
use serde::Deserialize;

pub struct ReadHtmlTool;

#[derive(Deserialize, JsonSchema)]
pub struct ReadHtmlParams {
    pub file_path: String,
}

#[async_trait]
impl TypedTool<ReadHtmlParams> for ReadHtmlTool {
    fn name(&self) -> &str {
        "ReadHtml"
    }

    fn description(&self) -> &str {
        "Reads an HTML file and converts it to plain text (markdown-like).\n\n\
         Usage:\n\
         - The file_path must be an absolute path to an .html or .htm file.\n\
         - HTML tags are stripped and content is converted to readable plain text.\n\
         - Useful for extracting text content from web pages saved locally."
    }

    fn permission(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn secret_eligible_params(&self) -> &'static [&'static str] {
        &[]
    }

    async fn execute(
        &self,
        input: ReadHtmlParams,
        ctx: &ToolContext,
    ) -> Result<ToolResult, LoopalError> {
        let file_path = &input.file_path;

        let path = match ctx.backend.resolve_path(file_path, false) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if !ext.eq_ignore_ascii_case("html") && !ext.eq_ignore_ascii_case("htm") {
            return Ok(ToolResult::error(
                "ReadHtml only supports .html/.htm files. Use Read for other file types.",
            ));
        }

        let raw = match ctx.backend.read_raw(&path).await {
            Ok(s) => s,
            Err(e) => return Ok(ToolResult::error(e.to_string())),
        };

        let converted = html2text::from_read(raw.as_bytes(), 120);
        Ok(ToolResult::success(converted))
    }
}
